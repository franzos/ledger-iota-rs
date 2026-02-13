use ledger_iota::{Bip32Path, LedgerIota, TransportType};

fn main() {
    #[cfg(feature = "hid")]
    {
        let ledger = LedgerIota::new(&TransportType::NativeHID).unwrap_or_else(|e| {
            eprintln!("failed to connect: {e}");
            std::process::exit(1);
        });
        let path = Bip32Path::iota(0, 0, 0);
        let (pubkey, address) = ledger.get_pubkey(&path).expect("failed to get pubkey");
        println!("path:    {path}");
        println!("pubkey:  {pubkey}");
        println!("address: {address}");
    }
    #[cfg(not(feature = "hid"))]
    {
        eprintln!("enable the 'hid' feature to use USB transport");
    }
}
