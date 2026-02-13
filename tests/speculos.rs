//! Integration tests — requires a running Speculos instance with the IOTA app:
//!
//! ```sh
//! speculos --model nanosp /path/to/app-iota.elf
//! ```
//!
//! Then: `cargo test --features tcp -- --ignored`

#![cfg(feature = "tcp")]

use ledger_iota::{Bip32Path, LedgerIota, TransportType};

fn connect() -> LedgerIota {
    let host = std::env::var("LEDGER_TCP_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let transport = TransportType::TCP(host, 9999);
    LedgerIota::new(&transport).expect("failed to connect to Speculos — is it running?")
}

#[test]
#[ignore = "requires Speculos"]
fn get_version() {
    let ledger = connect();
    let version = ledger.get_version().unwrap();
    assert_eq!(version.name.to_lowercase(), "iota");
    assert!((version.major, version.minor) >= (0, 9));
}

#[test]
#[ignore = "requires Speculos"]
fn is_app_open() {
    let ledger = connect();
    assert!(ledger.is_app_open());
}

#[test]
#[ignore = "requires Speculos"]
fn get_pubkey_default_path() {
    let ledger = connect();
    let path = Bip32Path::iota(0, 0, 0);
    let (pubkey, address) = ledger.get_pubkey(&path).unwrap();
    let pk_bytes: &[u8] = pubkey.as_ref();
    let addr_bytes: &[u8] = address.as_ref();
    assert_eq!(pk_bytes.len(), 32);
    assert_eq!(addr_bytes.len(), 32);
    // all zeros means derivation failed
    assert!(pk_bytes.iter().any(|&b| b != 0));
}

#[test]
#[ignore = "requires Speculos"]
fn get_pubkey_different_paths_differ() {
    let ledger = connect();
    let (pk1, _) = ledger.get_pubkey(&Bip32Path::iota(0, 0, 0)).unwrap();
    let (pk2, _) = ledger.get_pubkey(&Bip32Path::iota(0, 0, 1)).unwrap();
    assert_ne!(pk1, pk2);
}

#[test]
#[ignore = "requires Speculos"]
fn get_pubkey_testnet_path() {
    let ledger = connect();
    let path = Bip32Path::testnet(0, 0, 0);
    let (pubkey, _) = ledger.get_pubkey(&path).unwrap();
    let pk_bytes: &[u8] = pubkey.as_ref();
    assert!(pk_bytes.iter().any(|&b| b != 0));
}

#[test]
#[ignore = "requires Speculos"]
fn get_pubkey_deterministic() {
    let ledger = connect();
    let path = Bip32Path::iota(0, 0, 0);
    let (pk1, addr1) = ledger.get_pubkey(&path).unwrap();
    let (pk2, addr2) = ledger.get_pubkey(&path).unwrap();
    assert_eq!(pk1, pk2);
    assert_eq!(addr1, addr2);
}
