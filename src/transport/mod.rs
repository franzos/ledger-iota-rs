//! Transport backends for talking to Ledger devices.
//!
//! - [`hid::HidTransport`] -- USB HID for real hardware (feature `hid`, default)
//! - [`tcp::TcpTransport`] -- TCP for the Speculos simulator (feature `tcp`)

#[cfg(feature = "hid")]
pub mod hid;
#[cfg(feature = "tcp")]
pub mod tcp;

use crate::apdu::{ApduAnswer, ApduCommand};
use crate::error::TransportError;

pub trait Transport: Send + Sync {
    fn exchange(&self, command: &ApduCommand) -> Result<ApduAnswer, TransportError>;
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum TransportType {
    #[cfg(feature = "hid")]
    NativeHID,
    /// `(host, port)` for the Speculos simulator.
    #[cfg(feature = "tcp")]
    TCP(String, u16),
}

pub fn open(transport_type: &TransportType) -> Result<Box<dyn Transport>, TransportError> {
    match transport_type {
        #[cfg(feature = "hid")]
        TransportType::NativeHID => {
            let t = hid::HidTransport::new()?;
            Ok(Box::new(t))
        }
        #[cfg(feature = "tcp")]
        TransportType::TCP(host, port) => {
            let t = tcp::TcpTransport::new(host, *port)?;
            Ok(Box::new(t))
        }
        #[allow(unreachable_patterns)]
        _ => Err(TransportError::Comm(
            "no transport enabled — enable the 'hid' or 'tcp' feature".into(),
        )),
    }
}
