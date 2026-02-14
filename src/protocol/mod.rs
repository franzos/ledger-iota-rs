//! Block protocol for sending large payloads to the Ledger.
//!
//! Data gets split into 180-byte blocks linked by SHA256 hashes. The host
//! sends the first-block hashes, then the device pulls blocks by hash
//! until it has enough to produce a result.

pub mod chunks;

use std::collections::HashMap;

use crate::apdu::{ApduAnswer, ApduCommand, Instruction};
use crate::error::{LedgerError, StatusWord};
use crate::transport::Transport;
use chunks::{build_block_chain, Block};

#[repr(u8)]
enum HostMsg {
    Start = 0x00,
    GetChunkResponseSuccess = 0x01,
    GetChunkResponseFailure = 0x02,
    PutChunkResponse = 0x03,
    ResultAccumulatingResponse = 0x04,
}

#[repr(u8)]
enum DeviceMsg {
    ResultAccumulating = 0x00,
    ResultFinal = 0x01,
    GetChunk = 0x02,
    PutChunk = 0x03,
}

/// Run the block protocol for a given instruction.
///
/// Each parameter gets chunked into 180-byte SHA256-linked blocks.
/// We send the first-block hashes, then respond to device requests
/// until it yields a final result.
pub fn execute(
    transport: &dyn Transport,
    ins: Instruction,
    params: &[Vec<u8>],
) -> Result<Vec<u8>, LedgerError> {
    let mut hash_map: HashMap<[u8; 32], Block> = HashMap::new();
    let mut first_hashes: Vec<[u8; 32]> = Vec::new();

    for param in params {
        let blocks = build_block_chain(param);
        if let Some(first) = blocks.first() {
            first_hashes.push(chunks::hash_block(first));
        }
        for block in blocks {
            let h = chunks::hash_block(&block);
            hash_map.insert(h, block);
        }
    }

    // Device can also push chunks back to us via PUT_CHUNK
    let mut put_store: HashMap<[u8; 32], Vec<u8>> = HashMap::new();

    let mut start_data = Vec::with_capacity(1 + first_hashes.len() * 32);
    start_data.push(HostMsg::Start as u8);
    for h in &first_hashes {
        start_data.extend_from_slice(h);
    }

    let mut result = Vec::new();
    let mut response = send_apdu(transport, ins, start_data)?;

    loop {
        let data = response.data();
        if data.is_empty() {
            let code = response.retcode();
            if code != 0 && !StatusWord::is_success(code) {
                return Err(LedgerError::from_status(code));
            }
            return Err(LedgerError::BlockProtocol("empty response".into()));
        }

        match data[0] {
            x if x == DeviceMsg::ResultFinal as u8 => {
                result.extend_from_slice(&data[1..]);
                return Ok(result);
            }
            x if x == DeviceMsg::ResultAccumulating as u8 => {
                result.extend_from_slice(&data[1..]);
                let ack = vec![HostMsg::ResultAccumulatingResponse as u8];
                response = send_apdu(transport, ins, ack)?;
            }
            x if x == DeviceMsg::GetChunk as u8 => {
                if data.len() < 33 {
                    return Err(LedgerError::BlockProtocol(
                        "GET_CHUNK response too short".into(),
                    ));
                }
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&data[1..33]);

                if let Some(block) = hash_map.get(&hash) {
                    let mut reply = Vec::with_capacity(1 + block.serialized_len());
                    reply.push(HostMsg::GetChunkResponseSuccess as u8);
                    block.serialize_into(&mut reply);
                    response = send_apdu(transport, ins, reply)?;
                } else if let Some(stored) = put_store.get(&hash) {
                    let mut reply = Vec::with_capacity(1 + stored.len());
                    reply.push(HostMsg::GetChunkResponseSuccess as u8);
                    reply.extend_from_slice(stored);
                    response = send_apdu(transport, ins, reply)?;
                } else {
                    let reply = vec![HostMsg::GetChunkResponseFailure as u8];
                    response = send_apdu(transport, ins, reply)?;
                }
            }
            x if x == DeviceMsg::PutChunk as u8 => {
                let chunk_data = data[1..].to_vec();
                let hash = chunks::sha256(&chunk_data);
                put_store.insert(hash, chunk_data);
                let ack = vec![HostMsg::PutChunkResponse as u8];
                response = send_apdu(transport, ins, ack)?;
            }
            other => {
                return Err(LedgerError::BlockProtocol(format!(
                    "unknown device message type: 0x{other:02X}"
                )));
            }
        }
    }
}

/// The block protocol has its own flow control via message type bytes,
/// so the SW is irrelevant during exchanges -- matches the reference
/// Python client.
fn send_apdu(
    transport: &dyn Transport,
    ins: Instruction,
    data: Vec<u8>,
) -> Result<ApduAnswer, LedgerError> {
    let cmd = ApduCommand::with_data(ins as u8, data);
    let answer = transport.exchange(&cmd).map_err(LedgerError::Transport)?;
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Mock transport that returns a sequence of pre-built APDU responses.
    struct MockTransport {
        responses: Mutex<Vec<Vec<u8>>>,
    }

    impl MockTransport {
        fn new(responses: Vec<Vec<u8>>) -> Self {
            Self {
                responses: Mutex::new(responses),
            }
        }
    }

    impl Transport for MockTransport {
        fn exchange(&self, _cmd: &ApduCommand) -> Result<ApduAnswer, crate::error::TransportError> {
            let mut q = self.responses.lock().unwrap();
            if q.is_empty() {
                panic!("MockTransport: no more responses");
            }
            Ok(ApduAnswer::from_raw(q.remove(0)))
        }
    }

    /// Build a raw APDU response: `[payload][SW1][SW2]`
    fn apdu_ok(payload: &[u8]) -> Vec<u8> {
        let mut v = payload.to_vec();
        v.push(0x90);
        v.push(0x00);
        v
    }

    #[test]
    fn result_final_no_params() {
        // Device immediately returns ResultFinal with payload "hello"
        let mut payload = vec![DeviceMsg::ResultFinal as u8];
        payload.extend_from_slice(b"hello");
        let transport = MockTransport::new(vec![apdu_ok(&payload)]);

        let result = execute(&transport, Instruction::GetVersion, &[]).unwrap();
        assert_eq!(result, b"hello");
    }

    #[test]
    fn result_final_empty_payload() {
        let payload = vec![DeviceMsg::ResultFinal as u8];
        let transport = MockTransport::new(vec![apdu_ok(&payload)]);

        let result = execute(&transport, Instruction::GetVersion, &[]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn result_accumulating_then_final() {
        // First response: ResultAccumulating with "part1"
        let mut resp1 = vec![DeviceMsg::ResultAccumulating as u8];
        resp1.extend_from_slice(b"part1");

        // Second response: ResultFinal with "part2"
        let mut resp2 = vec![DeviceMsg::ResultFinal as u8];
        resp2.extend_from_slice(b"part2");

        let transport = MockTransport::new(vec![apdu_ok(&resp1), apdu_ok(&resp2)]);

        let result = execute(&transport, Instruction::GetVersion, &[]).unwrap();
        assert_eq!(result, b"part1part2");
    }

    #[test]
    fn get_chunk_serves_block_from_params() {
        // Provide a small param so there's one block in the hash_map
        let param = b"test_data".to_vec();
        let blocks = chunks::build_block_chain(&param);
        let hash = chunks::hash_block(&blocks[0]);

        // Device asks for that block hash, then returns ResultFinal
        let mut get_chunk = vec![DeviceMsg::GetChunk as u8];
        get_chunk.extend_from_slice(&hash);

        let mut final_resp = vec![DeviceMsg::ResultFinal as u8];
        final_resp.extend_from_slice(b"ok");

        let transport = MockTransport::new(vec![apdu_ok(&get_chunk), apdu_ok(&final_resp)]);

        let result = execute(&transport, Instruction::GetVersion, &[param]).unwrap();
        assert_eq!(result, b"ok");
    }

    #[test]
    fn get_chunk_unknown_hash_sends_failure() {
        // Device asks for a hash we don't have
        let mut get_chunk = vec![DeviceMsg::GetChunk as u8];
        get_chunk.extend_from_slice(&[0xAA; 32]); // unknown hash

        // After failure response, device returns ResultFinal
        let mut final_resp = vec![DeviceMsg::ResultFinal as u8];
        final_resp.extend_from_slice(b"done");

        let transport = MockTransport::new(vec![apdu_ok(&get_chunk), apdu_ok(&final_resp)]);

        let result = execute(&transport, Instruction::GetVersion, &[]).unwrap();
        assert_eq!(result, b"done");
    }

    #[test]
    fn get_chunk_too_short_errors() {
        // GetChunk with only 10 bytes of hash (need 32)
        let mut get_chunk = vec![DeviceMsg::GetChunk as u8];
        get_chunk.extend_from_slice(&[0xBB; 10]);

        let transport = MockTransport::new(vec![apdu_ok(&get_chunk)]);

        let err = execute(&transport, Instruction::GetVersion, &[]).unwrap_err();
        assert!(matches!(err, LedgerError::BlockProtocol(_)));
    }

    #[test]
    fn put_chunk_stores_and_can_be_retrieved() {
        let chunk_data = b"device_pushed_this";
        let chunk_hash = chunks::sha256(chunk_data);

        // 1. Device pushes a chunk
        let mut put_msg = vec![DeviceMsg::PutChunk as u8];
        put_msg.extend_from_slice(chunk_data);

        // 2. Device asks for the chunk back by hash
        let mut get_msg = vec![DeviceMsg::GetChunk as u8];
        get_msg.extend_from_slice(&chunk_hash);

        // 3. Device returns final
        let mut final_resp = vec![DeviceMsg::ResultFinal as u8];
        final_resp.extend_from_slice(b"ok");

        let transport = MockTransport::new(vec![
            apdu_ok(&put_msg),
            apdu_ok(&get_msg),
            apdu_ok(&final_resp),
        ]);

        let result = execute(&transport, Instruction::GetVersion, &[]).unwrap();
        assert_eq!(result, b"ok");
    }

    #[test]
    fn unknown_message_type_errors() {
        let payload = vec![0xFF]; // not a known DeviceMsg
        let transport = MockTransport::new(vec![apdu_ok(&payload)]);

        let err = execute(&transport, Instruction::GetVersion, &[]).unwrap_err();
        assert!(matches!(err, LedgerError::BlockProtocol(_)));
    }

    #[test]
    fn empty_response_with_error_status() {
        // Empty payload + error status word (0x6985 = UserRejected)
        let transport = MockTransport::new(vec![vec![0x69, 0x85]]);

        let err = execute(&transport, Instruction::GetVersion, &[]).unwrap_err();
        assert!(matches!(err, LedgerError::UserRejected));
    }

    #[test]
    fn empty_response_with_success_status_still_errors() {
        // Empty payload + 0x9000 — no data means protocol error
        let transport = MockTransport::new(vec![vec![0x90, 0x00]]);

        let err = execute(&transport, Instruction::GetVersion, &[]).unwrap_err();
        assert!(matches!(err, LedgerError::BlockProtocol(_)));
    }

    #[test]
    fn transport_error_propagates() {
        struct FailTransport;
        impl Transport for FailTransport {
            fn exchange(
                &self,
                _cmd: &ApduCommand,
            ) -> Result<ApduAnswer, crate::error::TransportError> {
                Err(crate::error::TransportError::Timeout(5000))
            }
        }

        let err = execute(&FailTransport, Instruction::GetVersion, &[]).unwrap_err();
        assert!(matches!(err, LedgerError::Transport(_)));
    }
}
