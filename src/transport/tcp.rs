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
