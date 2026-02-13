//! Core types: BIP32 derivation paths, app version, public key, address, signature.

use crate::error::LedgerError;
use byteorder::{LittleEndian, WriteBytesExt};

const HARDENED: u32 = 0x8000_0000;

/// BIP32 derivation path for IOTA addresses.
///
/// All components must be hardened. Supports `44'/4218'/...` (mainnet)
/// or `44'/1'/...` (testnet).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bip32Path(Vec<u32>);

impl Bip32Path {
    /// Each component must already have the hardened bit set.
    pub fn new(components: Vec<u32>) -> Result<Self, LedgerError> {
        let path = Self(components);
        path.validate()?;
        Ok(path)
    }

    /// Mainnet: `44'/4218'/account'/change'/index'`
    #[must_use]
    pub fn iota(account: u32, change: u32, index: u32) -> Self {
        Self(vec![
            44 | HARDENED,
            4218 | HARDENED,
            account | HARDENED,
            change | HARDENED,
            index | HARDENED,
        ])
    }

    /// Testnet: `44'/1'/account'/change'/index'`
    #[must_use]
    pub fn testnet(account: u32, change: u32, index: u32) -> Self {
        Self(vec![
            44 | HARDENED,
            1 | HARDENED,
            account | HARDENED,
            change | HARDENED,
            index | HARDENED,
        ])
    }

    /// Wire format: `[n: u8][path[0]: u32 LE]...[path[n-1]: u32 LE]`
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + self.0.len() * 4);
        buf.push(self.0.len() as u8);
        for &component in &self.0 {
            buf.write_u32::<LittleEndian>(component).unwrap();
        }
        buf
    }

    pub fn components(&self) -> &[u32] {
        &self.0
    }

    fn validate(&self) -> Result<(), LedgerError> {
        if self.0.len() < 2 {
            return Err(LedgerError::InvalidPath(
                "path must have at least 2 components".into(),
            ));
        }

        if self.0[0] != (44 | HARDENED) {
            return Err(LedgerError::InvalidPath(
                "first component must be 44'".into(),
            ));
        }

        let coin = self.0[1];
        if coin != (4218 | HARDENED) && coin != (1 | HARDENED) {
            return Err(LedgerError::InvalidPath(
                "coin type must be 4218' (mainnet) or 1' (testnet)".into(),
            ));
        }

        for (i, &c) in self.0.iter().enumerate() {
            if c & HARDENED == 0 {
                return Err(LedgerError::InvalidPath(format!(
                    "component {i} must be hardened"
                )));
            }
        }

        Ok(())
    }
}

impl std::fmt::Display for Bip32Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "m")?;
        for &c in &self.0 {
            let val = c & !HARDENED;
            let h = if c & HARDENED != 0 { "'" } else { "" };
            write!(f, "/{val}{h}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub name: String,
}

impl std::fmt::Display for AppVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} v{}.{}.{}",
            self.name, self.major, self.minor, self.patch
        )
    }
}

/// 32-byte Ed25519 public key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey(pub [u8; 32]);

/// 32-byte Blake2b-256 address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address(pub [u8; 32]);

/// 64-byte Ed25519 signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature(pub [u8; 64]);

impl AsRef<[u8]> for PublicKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsRef<[u8]> for Signature {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Display for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

impl std::fmt::Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

#[cfg(feature = "iota-sdk-types")]
impl From<PublicKey> for iota_sdk_types::Ed25519PublicKey {
    fn from(pk: PublicKey) -> Self {
        Self::new(pk.0)
    }
}

#[cfg(feature = "iota-sdk-types")]
impl From<Address> for iota_sdk_types::Address {
    fn from(addr: Address) -> Self {
        Self::new(addr.0)
    }
}

#[cfg(feature = "iota-sdk-types")]
impl From<Signature> for iota_sdk_types::Ed25519Signature {
    fn from(sig: Signature) -> Self {
        Self::new(sig.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iota_mainnet_path() {
        let path = Bip32Path::iota(0, 0, 0);
        let bytes = path.serialize();
        assert_eq!(bytes[0], 5); // 5 components
                                 // 44' = 0x8000002C in little-endian
        assert_eq!(&bytes[1..5], &[0x2C, 0x00, 0x00, 0x80]);
        // 4218' = 0x8000107A in little-endian
        assert_eq!(&bytes[5..9], &[0x7A, 0x10, 0x00, 0x80]);
        assert_eq!(path.to_string(), "m/44'/4218'/0'/0'/0'");
    }

    #[test]
    fn testnet_path() {
        let path = Bip32Path::testnet(1, 0, 5);
        let bytes = path.serialize();
        assert_eq!(bytes[0], 5);
        // 1' = 0x80000001 in little-endian
        assert_eq!(&bytes[5..9], &[0x01, 0x00, 0x00, 0x80]);
        assert_eq!(path.to_string(), "m/44'/1'/1'/0'/5'");
    }

    #[test]
    fn serialization_length() {
        let path = Bip32Path::iota(0, 0, 0);
        let bytes = path.serialize();
        assert_eq!(bytes.len(), 1 + 5 * 4); // 1 byte count + 5 * 4 bytes
    }

    #[test]
    fn invalid_coin_type() {
        let result = Bip32Path::new(vec![44 | 0x80000000, 999 | 0x80000000]);
        assert!(result.is_err());
    }

    #[test]
    fn non_hardened_rejected() {
        let result = Bip32Path::new(vec![44 | 0x80000000, 4218 | 0x80000000, 0]);
        assert!(result.is_err());
    }

    #[test]
    fn too_short_rejected() {
        let result = Bip32Path::new(vec![44 | 0x80000000]);
        assert!(result.is_err());
    }
}
