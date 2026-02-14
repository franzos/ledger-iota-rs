//! Integration tests — requires a running Speculos instance with the IOTA app.
//!
//! ```sh
//! podman compose up -d
//! cargo test --features tcp -- --ignored
//! ```

#![cfg(feature = "tcp")]

use ledger_iota::{Bip32Path, LedgerError, LedgerIota, TransportType};

fn connect() -> LedgerIota {
    let host = std::env::var("LEDGER_TCP_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let transport = TransportType::TCP(host, 9999);
    LedgerIota::new(&transport).expect("failed to connect to Speculos — is it running?")
}

/// Base64-decode helper (avoids extra dev-dependency).
fn b64(s: &str) -> Vec<u8> {
    let mut dec = Vec::new();
    // simple base64 decode
    let lut: Vec<u8> = (0..256u16)
        .map(|i| {
            let c = i as u8;
            match c {
                b'A'..=b'Z' => c - b'A',
                b'a'..=b'z' => c - b'a' + 26,
                b'0'..=b'9' => c - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => 0xFF,
            }
        })
        .collect();

    let bytes: Vec<u8> = s
        .bytes()
        .filter(|&b| lut[b as usize] != 0xFF && b != b'=')
        .collect();
    for chunk in bytes.chunks(4) {
        let vals: Vec<u8> = chunk.iter().map(|&b| lut[b as usize]).collect();
        if vals.len() >= 2 {
            dec.push((vals[0] << 2) | (vals[1] >> 4));
        }
        if vals.len() >= 3 {
            dec.push((vals[1] << 4) | (vals[2] >> 2));
        }
        if vals.len() >= 4 {
            dec.push((vals[2] << 6) | vals[3]);
        }
    }
    dec
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

fn speculos_api_port() -> u16 {
    std::env::var("SPECULOS_API_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5000)
}

/// HTTP helper — sends a request to the Speculos REST API and returns the response body.
fn speculos_http(api_port: u16, method: &str, path: &str, body: Option<&str>) -> String {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let addr = format!("127.0.0.1:{api_port}");
    let mut s = TcpStream::connect(&addr).expect("cannot reach Speculos REST API");
    let req = if let Some(b) = body {
        format!(
            "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}",
            b.len()
        )
    } else {
        format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n")
    };
    s.write_all(req.as_bytes()).unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).unwrap();
    let resp = String::from_utf8_lossy(&buf);
    // Return just the body (after the blank line)
    resp.split("\r\n\r\n").nth(1).unwrap_or("").to_string()
}

/// Read the current screen text from the Speculos events API.
fn screen_text(api_port: u16) -> String {
    speculos_http(api_port, "GET", "/events?currentscreenonly=true", None)
}

/// Press a single button via the Speculos REST API.
fn press_button(api_port: u16, button: &str) {
    let path = format!("/button/{button}");
    speculos_http(
        api_port,
        "POST",
        &path,
        Some(r#"{"action":"press-and-release"}"#),
    );
    std::thread::sleep(std::time::Duration::from_millis(500));
}

/// Press a sequence of buttons via the Speculos REST API.
/// "R" = right, "B" = both, "L" = left.
fn press_buttons(api_port: u16, sequence: &str) {
    for ch in sequence.chars() {
        let button = match ch {
            'R' => "right",
            'B' => "both",
            'L' => "left",
            _ => continue,
        };
        press_button(api_port, button);
    }
}

/// Ensure blind signing is enabled. Checks the actual device state via the events
/// API and only toggles if needed, so it's safe to call multiple times.
fn ensure_blind_signing(api_port: u16) {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Navigate: Home → Right (Settings) → Both (enter settings)
        press_buttons(api_port, "RB");
        let text = screen_text(api_port);
        if text.contains("Enabled") {
            // Already enabled — go back: Right (Back) → Both (exit) → Left (home)
            press_buttons(api_port, "RBL");
        } else {
            // Toggle on: Both (toggle) → Right (Back) → Both (exit) → Left (home)
            press_buttons(api_port, "BRBL");
        }
    });
}

/// Blind-sign a transaction (no object data).
/// Requires Speculos with REST API.
#[test]
#[ignore = "requires Speculos"]
fn sign_blind() {
    let api_port = speculos_api_port();
    ensure_blind_signing(api_port);

    let ledger = connect();
    let path = Bip32Path::iota(0, 0, 1);

    // Whole-gas-coin transfer from the ragger test vectors
    let tx = b64("AAAAAAABACAbNmnjIYk+5Jw4egj8JR2//zfNKpgebEc6Wyr94Z02PgEBAQABAAAPWOsTUUVNYjpqQ2YZjWzVqkoSoSo8rvtQFHbgbYvVtgEe7Gwy7zZGczH7ewiLSssc5G9zY1QwjgP/bOkTpCc04oYAAAAAAAAAIMfrxsRDCzz35Y11q1PduRgCdN72Oxq1YZ+9twls29cSD1jrE1FFTWI6akNmGY1s1apKEqEqPK77UBR24G2L1bboAwAAAAAAAEBCDwAAAAAAAA==");

    // Approve in background while sign_tx blocks for user interaction.
    // Flow: Both (accept blind signing risk) → Right×3 (review/hash screens) → Both (sign)
    let handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        press_buttons(api_port, "BRRRB");
    });

    let sig = ledger.sign_tx(&tx, &path, None).unwrap();
    handle.join().unwrap();

    let sig_bytes: &[u8] = sig.as_ref();
    assert_eq!(sig_bytes.len(), 64);
    assert!(sig_bytes.iter().any(|&b| b != 0));
}

/// Sign the same transaction twice and verify deterministic signatures.
#[test]
#[ignore = "requires Speculos"]
fn sign_deterministic() {
    let api_port = speculos_api_port();
    ensure_blind_signing(api_port);

    let ledger = connect();
    let path = Bip32Path::iota(0, 0, 1);
    let tx = b64("AAAAAAABACAbNmnjIYk+5Jw4egj8JR2//zfNKpgebEc6Wyr94Z02PgEBAQABAAAPWOsTUUVNYjpqQ2YZjWzVqkoSoSo8rvtQFHbgbYvVtgEe7Gwy7zZGczH7ewiLSssc5G9zY1QwjgP/bOkTpCc04oYAAAAAAAAAIMfrxsRDCzz35Y11q1PduRgCdN72Oxq1YZ+9twls29cSD1jrE1FFTWI6akNmGY1s1apKEqEqPK77UBR24G2L1bboAwAAAAAAAEBCDwAAAAAAAA==");

    let handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        press_buttons(api_port, "BRRRB");
    });
    let sig1 = ledger.sign_tx(&tx, &path, None).unwrap();
    handle.join().unwrap();

    let handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        press_buttons(api_port, "BRRRB");
    });
    let sig2 = ledger.sign_tx(&tx, &path, None).unwrap();
    handle.join().unwrap();

    assert_eq!(sig1, sig2);
}

/// Verify the signature is cryptographically valid against the derived pubkey.
#[test]
#[ignore = "requires Speculos"]
fn sign_verify() {
    use blake2::{digest::consts::U32, Blake2b, Digest};
    use ed25519_dalek::{Signature as DalekSig, Verifier, VerifyingKey};

    let api_port = speculos_api_port();
    ensure_blind_signing(api_port);

    let ledger = connect();
    let path = Bip32Path::iota(0, 0, 1);
    let (pubkey, _) = ledger.get_pubkey(&path).unwrap();

    let tx = b64("AAAAAAABACAbNmnjIYk+5Jw4egj8JR2//zfNKpgebEc6Wyr94Z02PgEBAQABAAAPWOsTUUVNYjpqQ2YZjWzVqkoSoSo8rvtQFHbgbYvVtgEe7Gwy7zZGczH7ewiLSssc5G9zY1QwjgP/bOkTpCc04oYAAAAAAAAAIMfrxsRDCzz35Y11q1PduRgCdN72Oxq1YZ+9twls29cSD1jrE1FFTWI6akNmGY1s1apKEqEqPK77UBR24G2L1bboAwAAAAAAAEBCDwAAAAAAAA==");

    let handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        press_buttons(api_port, "BRRRB");
    });
    let sig = ledger.sign_tx(&tx, &path, None).unwrap();
    handle.join().unwrap();

    let pk_bytes: &[u8] = pubkey.as_ref();
    let sig_bytes: &[u8] = sig.as_ref();

    let vk = VerifyingKey::from_bytes(pk_bytes.try_into().unwrap()).unwrap();
    let dalek_sig = DalekSig::from_bytes(sig_bytes.try_into().unwrap());

    // IOTA signs Blake2b-256(intent_message)
    let digest = <Blake2b<U32> as Digest>::digest(&tx);
    vk.verify(digest.as_ref(), &dalek_sig)
        .expect("signature verification failed — device produced an invalid signature");
}

/// User rejection produces the expected error.
#[test]
#[ignore = "requires Speculos"]
fn sign_user_rejected() {
    let api_port = speculos_api_port();
    ensure_blind_signing(api_port);

    let ledger = connect();
    let path = Bip32Path::iota(0, 0, 1);
    let tx = b64("AAAAAAABACAbNmnjIYk+5Jw4egj8JR2//zfNKpgebEc6Wyr94Z02PgEBAQABAAAPWOsTUUVNYjpqQ2YZjWzVqkoSoSo8rvtQFHbgbYvVtgEe7Gwy7zZGczH7ewiLSssc5G9zY1QwjgP/bOkTpCc04oYAAAAAAAAAIMfrxsRDCzz35Y11q1PduRgCdN72Oxq1YZ+9twls29cSD1jrE1FFTWI6akNmGY1s1apKEqEqPK77UBR24G2L1bboAwAAAAAAAEBCDwAAAAAAAA==");

    // Reject: Both (accept blind signing warning) → Right (to Reject) → Both (confirm reject)
    // The exact button sequence depends on the app UI; we navigate past the
    // blind-sign warning, then scroll to the reject option and confirm it.
    let handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        // Navigate through review screens until we find reject
        press_buttons(api_port, "BRRRRB");
    });

    let err = ledger.sign_tx(&tx, &path, None).unwrap_err();
    handle.join().unwrap();

    assert!(
        matches!(err, LedgerError::UserRejected),
        "expected UserRejected, got: {err:?}"
    );
}

/// verify_address returns the same key pair as get_pubkey for the same path.
#[test]
#[ignore = "requires Speculos"]
fn verify_address_matches_pubkey() {
    let api_port = speculos_api_port();

    let ledger = connect();
    let path = Bip32Path::iota(0, 0, 0);

    let (pk_get, addr_get) = ledger.get_pubkey(&path).unwrap();

    // verify_address shows the address on-screen and blocks until user confirms.
    // R×3: Verify → Address 1/2 → Address 2/2 → Confirm, then B to approve
    let handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        press_buttons(api_port, "RRRB");
    });

    let (pk_verify, addr_verify) = ledger.verify_address(&path).unwrap();
    handle.join().unwrap();

    assert_eq!(pk_get, pk_verify);
    assert_eq!(addr_get, addr_verify);
}

/// Sign a message and verify the signature.
#[test]
#[ignore = "requires Speculos"]
fn sign_message() {
    use blake2::{digest::consts::U32, Blake2b, Digest};
    use ed25519_dalek::{Signature as DalekSig, Verifier, VerifyingKey};

    let api_port = speculos_api_port();

    let ledger = connect();
    let path = Bip32Path::iota(0, 0, 1);
    let (pubkey, _) = ledger.get_pubkey(&path).unwrap();

    let message = b"Hello";

    // Personal message flow: Review → Message → Sign
    let handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        press_buttons(api_port, "RRB");
    });

    let sig = ledger.sign_message(message, &path).unwrap();
    handle.join().unwrap();

    let sig_bytes: &[u8] = sig.as_ref();
    assert_eq!(sig_bytes.len(), 64);

    // Verify: signature covers Blake2b-256([3, 0, 0] || message)
    let pk_bytes: &[u8] = pubkey.as_ref();
    let vk = VerifyingKey::from_bytes(pk_bytes.try_into().unwrap()).unwrap();
    let dalek_sig = DalekSig::from_bytes(sig_bytes.try_into().unwrap());

    let mut intent_message = vec![3u8, 0, 0];
    intent_message.extend_from_slice(message);
    let digest = <Blake2b<U32> as Digest>::digest(&intent_message);
    vk.verify(digest.as_ref(), &dalek_sig)
        .expect("message signature verification failed");
}

/// Sign a transaction large enough to require multi-block chunking (>180 bytes).
#[test]
#[ignore = "requires Speculos"]
fn sign_large_tx() {
    use ledger_iota::{build_transfer_tx, GasCoinRef};

    let api_port = speculos_api_port();
    ensure_blind_signing(api_port);

    let ledger = connect();
    let path = Bip32Path::iota(0, 0, 1);

    // build_transfer_tx produces ~200 bytes which crosses the 180-byte block
    // boundary, forcing the protocol to chunk into multiple blocks.
    let sender = [0x01; 32];
    let recipient = [0x02; 32];
    let gas = GasCoinRef {
        object_id: [0xAA; 32],
        version: 1,
        digest: [0xBB; 32],
    };
    let tx = build_transfer_tx(&sender, &recipient, 1_000_000, &gas, 10_000_000, 1000);
    assert!(
        tx.len() > 180,
        "TX must exceed single block size for this test"
    );

    // Clear-sign flow (device parses the transfer TX):
    // R×7: Review → From 1/2 → From 2/2 → To 1/2 → To 2/2 → Amount → Max Gas → Sign
    // B: approve
    let handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        press_buttons(api_port, "RRRRRRRB");
    });

    let sig = ledger.sign_tx(&tx, &path, None).unwrap();
    handle.join().unwrap();

    let sig_bytes: &[u8] = sig.as_ref();
    assert_eq!(sig_bytes.len(), 64);
    assert!(sig_bytes.iter().any(|&b| b != 0));
}
