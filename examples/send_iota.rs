//! Sign a simple IOTA transfer on a real Ledger device.
//!
//! Usage:
//!   cargo run --example send_iota -- <RECIPIENT> <AMOUNT_NANOS>
//!
//! Example:
//!   cargo run --example send_iota -- 0xabc...def 1000000000
//!
//! NOTE: gas coin data is placeholder — replace with real values from your
//! wallet / RPC before broadcasting.

use std::env;

use ledger_iota::{build_transfer_tx, Bip32Path, GasCoinRef, LedgerIota, TransportType};

fn main() {
    #[cfg(feature = "hid")]
    {
        let args: Vec<String> = env::args().collect();
        if args.len() != 3 {
            eprintln!("usage: send_iota <RECIPIENT_0x...> <AMOUNT_NANOS>");
            std::process::exit(1);
        }

        let recipient = parse_address(&args[1]);
        let amount: u64 = args[2].parse().unwrap_or_else(|_| {
            eprintln!("amount must be a u64 (nanos)");
            std::process::exit(1);
        });

        let ledger = LedgerIota::new(&TransportType::NativeHID).unwrap_or_else(|e| {
            eprintln!("failed to connect: {e}");
            std::process::exit(1);
        });
        let path = Bip32Path::iota(0, 0, 0);
        let (_, sender_addr) = ledger
            .get_pubkey(&path)
            .expect("failed to get sender address");

        let mut sender = [0u8; 32];
        sender.copy_from_slice(sender_addr.as_ref());

        println!("sender:    0x{}", hex::encode(sender));
        println!("recipient: 0x{}", hex::encode(recipient));
        println!("amount:    {} nanos", amount);

        // TODO: replace with real gas coin from RPC (iota_getCoins / iota_getGasPrice)
        let gas = GasCoinRef {
            object_id: [0u8; 32],
            version: 1,
            digest: [0u8; 32],
        };
        let gas_budget: u64 = 10_000_000; // 0.01 IOTA
        let gas_price: u64 = 1000;

        let tx = build_transfer_tx(&sender, &recipient, amount, &gas, gas_budget, gas_price);

        println!("tx bytes:  {} bytes", tx.len());
        match ledger.sign_tx(&tx, &path, None) {
            Ok(sig) => println!("signature: {sig}"),
            Err(e) => eprintln!("signing failed: {e}"),
        }
    }
    #[cfg(not(feature = "hid"))]
    {
        eprintln!("enable the 'hid' feature to use USB transport");
    }
}

fn parse_address(s: &str) -> [u8; 32] {
    let hex_str = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(hex_str).unwrap_or_else(|e| {
        eprintln!("invalid hex address: {e}");
        std::process::exit(1);
    });
    if bytes.len() != 32 {
        eprintln!(
            "address must be 32 bytes (64 hex chars), got {}",
            bytes.len()
        );
        std::process::exit(1);
    }
    let mut addr = [0u8; 32];
    addr.copy_from_slice(&bytes);
    addr
}
