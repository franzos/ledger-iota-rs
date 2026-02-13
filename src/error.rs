//! Error types and Ledger status word mapping.

use thiserror::Error;

/// Raw status words returned by the Ledger device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum StatusWord {
    Ok = 0x9000,
    DeviceLocked = 0x5515,
    BlindSigningDisabled = 0x6808,
    NothingReceived = 0x6982,
    UserRejected = 0x6985,
    GeneralError = 0x6D00,
    WrongApp = 0x6E00,
    AppNotOpen = 0x6E01,
}

impl StatusWord {
    pub(crate) fn is_success(code: u16) -> bool {
        code == Self::Ok as u16
    }
}

/// Errors returned by the library.
#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),

    #[error("device returned status 0x{0:04X}: {1}")]
    DeviceStatus(u16, &'static str),

    #[error("device is locked or asleep — unlock it and open the IOTA app")]
    DeviceLocked,

    #[error("IOTA app is not open — open it and try again")]
    AppNotOpen,

    #[error("wrong app open on device (found {0}) — close it and open the IOTA app")]
    WrongApp(String),

    #[error("blind signing is disabled — enable it in the IOTA app settings")]
    BlindSigningDisabled,

    #[error("user rejected the request on device")]
    UserRejected,

    #[error("invalid response: {0}")]
    InvalidResponse(String),

    #[error("invalid BIP32 path: {0}")]
    InvalidPath(String),

    #[error("block protocol error: {0}")]
    BlockProtocol(String),
}

impl LedgerError {
    pub fn from_status(code: u16) -> Self {
        match code {
            c if c == StatusWord::DeviceLocked as u16 => Self::DeviceLocked,
            c if c == StatusWord::BlindSigningDisabled as u16 => Self::BlindSigningDisabled,
            c if c == StatusWord::NothingReceived as u16 => {
                Self::DeviceStatus(code, "nothing received")
            }
            c if c == StatusWord::UserRejected as u16 || c == StatusWord::GeneralError as u16 => {
                Self::UserRejected
            }
            c if c == StatusWord::WrongApp as u16 => Self::WrongApp("unknown".into()),
            c if c == StatusWord::AppNotOpen as u16 => Self::AppNotOpen,
            _ => Self::DeviceStatus(code, "unknown"),
        }
    }
}

/// Transport-level errors (USB, TCP, IO).
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("no Ledger device found — is it plugged in?")]
    DeviceNotFound,

    #[error("communication error: {0}")]
    Comm(String),

    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    #[error("device timed out after {0}ms")]
    Timeout(u32),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
