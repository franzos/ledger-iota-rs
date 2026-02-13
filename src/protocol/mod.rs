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
