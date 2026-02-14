//! High-level API - [`LedgerIota`] wraps a transport connection and
//! exposes all supported operations.

use crate::commands;
use crate::error::LedgerError;
use crate::objects::{self, ObjectData};
use crate::transport::{self, Transport, TransportType};
use crate::types::{AppVersion, Bip32Path};

#[cfg(not(feature = "iota-sdk-types"))]
use crate::types::{Address, PublicKey, Signature};

#[cfg(feature = "iota-sdk-types")]
type PublicKey = iota_sdk_types::Ed25519PublicKey;
#[cfg(feature = "iota-sdk-types")]
type Address = iota_sdk_types::Address;
#[cfg(feature = "iota-sdk-types")]
type Signature = iota_sdk_types::Ed25519Signature;

const MIN_VERSION: (u8, u8, u8) = (0, 9, 0);

fn is_iota_app(name: &str) -> bool {
    name.to_ascii_lowercase().contains("iota")
}

/// High-level interface to the IOTA Ledger app.
///
/// Wraps a transport connection (USB HID or TCP) and exposes
/// all supported operations: key derivation, address verification,
/// transaction signing.
pub struct LedgerIota {
    transport: Box<dyn Transport>,
}

impl LedgerIota {
    /// Connect to a Ledger device and verify the IOTA app is open.
    pub fn new(transport_type: &TransportType) -> Result<Self, LedgerError> {
        let transport = transport::open(transport_type)?;
        let ledger = Self { transport };

        let version = ledger.get_version()?;
        if !is_iota_app(&version.name) {
            return Err(LedgerError::WrongApp(version.name));
        }
        if !version_ok(&version) {
            return Err(LedgerError::InvalidResponse(format!(
                "app {version} is too old - update to at least {}.{}.{}",
                MIN_VERSION.0, MIN_VERSION.1, MIN_VERSION.2,
            )));
        }

        Ok(ledger)
    }

    /// Useful for testing or injecting a custom transport.
    pub fn with_transport(transport: Box<dyn Transport>) -> Self {
        Self { transport }
    }

    /// Query the app version and name from the device.
    pub fn get_version(&self) -> Result<AppVersion, LedgerError> {
        commands::get_version::exec(self.transport.as_ref())
    }

    /// Derive the public key and address for the given BIP32 path.
    pub fn get_pubkey(&self, path: &Bip32Path) -> Result<(PublicKey, Address), LedgerError> {
        let (pk, addr) = commands::get_pubkey::exec(self.transport.as_ref(), path)?;
        Ok((pk.into(), addr.into()))
    }

    /// Shows the address on device and waits for user confirmation.
    pub fn verify_address(&self, path: &Bip32Path) -> Result<(PublicKey, Address), LedgerError> {
        let (pk, addr) = commands::verify_address::exec(self.transport.as_ref(), path)?;
        Ok((pk.into(), addr.into()))
    }

    /// Pass `objects` to enable clear signing for non-standard tokens.
    pub fn sign_tx(
        &self,
        tx: &[u8],
        path: &Bip32Path,
        objects: Option<&[ObjectData]>,
    ) -> Result<Signature, LedgerError> {
        let encoded_objects = objects.map(objects::encode_objects);
        let sig = commands::sign_tx::exec(
            self.transport.as_ref(),
            tx,
            path,
            encoded_objects.as_deref(),
        )?;
        Ok(sig.into())
    }

    /// Tell the IOTA app to quit (the device goes back to the dashboard).
    pub fn quit(&self) -> Result<(), LedgerError> {
        commands::quit::exec(self.transport.as_ref())
    }

    /// Check whether the IOTA app is currently open on the device.
    pub fn is_app_open(&self) -> bool {
        match self.get_version() {
            Ok(v) => is_iota_app(&v.name),
            Err(_) => false,
        }
    }
}

fn version_ok(v: &AppVersion) -> bool {
    (v.major, v.minor, v.patch) >= MIN_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(major: u8, minor: u8, patch: u8) -> AppVersion {
        AppVersion {
            major,
            minor,
            patch,
            name: "iota".into(),
        }
    }

    #[test]
    fn version_ok_exact_minimum() {
        assert!(version_ok(&version(0, 9, 0)));
    }

    #[test]
    fn version_ok_above_minimum() {
        assert!(version_ok(&version(0, 9, 1)));
        assert!(version_ok(&version(0, 10, 0)));
        assert!(version_ok(&version(1, 0, 0)));
    }

    #[test]
    fn version_ok_below_minimum() {
        assert!(!version_ok(&version(0, 8, 9)));
        assert!(!version_ok(&version(0, 8, 255)));
        assert!(!version_ok(&version(0, 0, 0)));
    }
}
