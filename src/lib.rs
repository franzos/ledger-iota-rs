//! Rust client for the IOTA Rebased Ledger app (`app-iota` v1.0.x).
//!
//! Talks to the Ledger hardware wallet over USB HID or TCP (Speculos simulator).
//!
//! # Quick start
//!
//! ```no_run
//! use ledger_iota::{LedgerIota, Bip32Path, TransportType};
//!
//! let ledger = LedgerIota::new(&TransportType::NativeHID)?;
//!
//! let version = ledger.get_version()?;
//! println!("{version}");
//!
//! let path = Bip32Path::iota(0, 0, 0);
//! let (pubkey, address) = ledger.get_pubkey(&path)?;
//! println!("address: {address}");
//! # Ok::<(), ledger_iota::LedgerError>(())
//! ```
//!
//! # Modules
//!
//! - [`api`] -- high-level [`LedgerIota`] facade
//! - [`transport`] -- device communication (USB HID, TCP)
//! - [`objects`] -- object data encoding for clear signing
//! - [`tx`] -- transaction building helpers ([`build_transfer_tx`])
//! - [`types`] -- [`Bip32Path`], [`AppVersion`], [`PublicKey`], [`Address`], [`Signature`]
//!
//! # Feature flags
//!
//! - `hid` (default) -- USB HID transport for real Ledger devices
//! - `tcp` -- TCP transport for the Speculos simulator
//! - `iota-sdk-types` -- return [`iota_sdk_types`] types from `get_pubkey`/`sign_tx`
//!   instead of the built-in [`PublicKey`], [`Address`], [`Signature`] wrappers

pub(crate) mod apdu;
pub mod api;
pub(crate) mod commands;
pub mod error;
pub mod objects;
pub(crate) mod protocol;
pub mod transport;
pub mod tx;
pub mod types;

pub use api::LedgerIota;
pub use error::LedgerError;
#[cfg(feature = "iota-sdk-types")]
pub use iota_sdk_types::{Address, Ed25519PublicKey, Ed25519Signature};
pub use objects::{encode_objects, MoveObjectType, ObjectData, Owner, TypeTag};
#[cfg(feature = "hid")]
pub use transport::hid::DeviceType;
pub use transport::TransportType;
pub use tx::{build_transfer_tx, GasCoinRef};
#[cfg(not(feature = "iota-sdk-types"))]
pub use types::{Address, PublicKey, Signature};
pub use types::{AppVersion, Bip32Path};
