use ledger_iota::{Bip32Path, LedgerIota, TransportType};

fn main() {
    #[cfg(feature = "hid")]
    {
        let ledger = LedgerIota::new(&TransportType::NativeHID).unwrap_or_else(|e| {
            eprintln!("failed to connect: {e}");
            std::process::exit(1);
        });
        let path = Bip32Path::iota(0, 0, 0);
        println!("verify address on device for {path}...");
        let (pubkey, address) = ledger.verify_address(&path).expect("failed to verify");
        println!("pubkey:  {pubkey}");
        println!("address: {address}");
    }
    #[cfg(not(feature = "hid"))]
    {
        eprintln!("enable the 'hid' feature to use USB transport");
    }
}
