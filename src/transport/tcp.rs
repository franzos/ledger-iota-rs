use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;

use crate::apdu::{ApduAnswer, ApduCommand};
use crate::error::TransportError;
use crate::transport::Transport;

/// TCP transport for the Speculos simulator (default `127.0.0.1:9999`).
///
/// Wire: `[u32 BE length][APDU]` send, `[u32 BE length][response]` recv.
///
/// Speculos has a quirk: the status word (`SW1 SW2`) is sent as a bare
/// 2-byte suffix *outside* the length-prefixed frame, so we read both
/// and stitch them into a standard APDU response.
pub struct TcpTransport {
    stream: Mutex<TcpStream>,
}

impl TcpTransport {
    pub fn new(host: &str, port: u16) -> Result<Self, TransportError> {
        let addr = format!("{host}:{port}");
        let stream = TcpStream::connect(&addr)
            .map_err(|e| TransportError::ConnectionFailed(format!("{addr}: {e}")))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(30)))
            .map_err(TransportError::Io)?;
        Ok(Self {
            stream: Mutex::new(stream),
        })
    }
}

impl Transport for TcpTransport {
    fn exchange(&self, command: &ApduCommand) -> Result<ApduAnswer, TransportError> {
        let apdu = command.serialize();
        let mut stream = self
            .stream
            .lock()
            .map_err(|e| TransportError::Comm(format!("mutex poisoned: {e}")))?;

        let len = apdu.len() as u32;
        stream.write_all(&len.to_be_bytes())?;
        stream.write_all(&apdu)?;
        stream.flush()?;

        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf)?;
        let resp_len = u32::from_be_bytes(len_buf) as usize;
        if resp_len > 65536 {
            return Err(TransportError::Comm(format!(
                "response too large: {resp_len} bytes (max 65536)"
            )));
        }

        let mut resp = vec![0u8; resp_len + 2];
        stream.read_exact(&mut resp[..resp_len])?;

        // SW is sent bare after the framed data -- Speculos quirk
        stream.read_exact(&mut resp[resp_len..resp_len + 2])?;

        Ok(ApduAnswer::from_raw(resp))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// Spin up a local TCP listener, return (transport, server_stream).
    fn mock_pair() -> (TcpTransport, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let transport = TcpTransport::new("127.0.0.1", port).unwrap();
        let (server, _) = listener.accept().unwrap();
        (transport, server)
    }

    #[test]
    fn normal_exchange() {
        let (transport, mut server) = mock_pair();

        let cmd = ApduCommand::new(0x00);

        let handle = std::thread::spawn(move || transport.exchange(&cmd));

        // Read the request
        let mut len_buf = [0u8; 4];
        server.read_exact(&mut len_buf).unwrap();
        let req_len = u32::from_be_bytes(len_buf) as usize;
        let mut req = vec![0u8; req_len];
        server.read_exact(&mut req).unwrap();

        // Send response: [u32 BE length][payload][SW bare]
        let payload = b"\xAA\xBB";
        server
            .write_all(&(payload.len() as u32).to_be_bytes())
            .unwrap();
        server.write_all(payload).unwrap();
        server.write_all(&[0x90, 0x00]).unwrap(); // SW
        server.flush().unwrap();

        let answer = handle.join().unwrap().unwrap();
        assert_eq!(answer.retcode(), 0x9000);
        assert_eq!(answer.data(), &[0xAA, 0xBB]);
    }

    #[test]
    fn zero_length_response() {
        let (transport, mut server) = mock_pair();

        let cmd = ApduCommand::new(0x00);

        let handle = std::thread::spawn(move || transport.exchange(&cmd));

        // Consume request
        let mut len_buf = [0u8; 4];
        server.read_exact(&mut len_buf).unwrap();
        let req_len = u32::from_be_bytes(len_buf) as usize;
        let mut req = vec![0u8; req_len];
        server.read_exact(&mut req).unwrap();

        // Send zero-length response + SW
        server.write_all(&0u32.to_be_bytes()).unwrap();
        server.write_all(&[0x90, 0x00]).unwrap();
        server.flush().unwrap();

        let answer = handle.join().unwrap().unwrap();
        assert_eq!(answer.retcode(), 0x9000);
        assert!(answer.data().is_empty());
    }

    #[test]
    fn response_too_large_rejected() {
        let (transport, mut server) = mock_pair();

        let cmd = ApduCommand::new(0x00);

        let handle = std::thread::spawn(move || transport.exchange(&cmd));

        // Consume request
        let mut len_buf = [0u8; 4];
        server.read_exact(&mut len_buf).unwrap();
        let req_len = u32::from_be_bytes(len_buf) as usize;
        let mut req = vec![0u8; req_len];
        server.read_exact(&mut req).unwrap();

        // Claim response is 65537 bytes
        server.write_all(&65537u32.to_be_bytes()).unwrap();
        server.flush().unwrap();

        let err = handle.join().unwrap().unwrap_err();
        assert!(matches!(err, TransportError::Comm(_)));
    }

    #[test]
    fn connection_refused() {
        // Port 1 should be refused on most systems
        let result = TcpTransport::new("127.0.0.1", 1);
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(matches!(err, TransportError::ConnectionFailed(_)));
        }
    }
}
