use ledger_iota::{Bip32Path, LedgerIota, TransportType};

fn main() {
    #[cfg(feature = "hid")]
    {
        let ledger = LedgerIota::new(&TransportType::NativeHID).unwrap_or_else(|e| {
            eprintln!("failed to connect: {e}");
            std::process::exit(1);
        });

        for i in 0..5 {
            let path = Bip32Path::iota(0, 0, i);
            match ledger.get_pubkey(&path) {
                Ok((pubkey, address)) => {
                    println!("[{path}]");
                    println!("  pubkey:  {pubkey}");
                    println!("  address: {address}");
                }
                Err(e) => eprintln!("  error: {e}"),
            }
        }
    }
    #[cfg(not(feature = "hid"))]
    {
        eprintln!("enable the 'hid' feature to use USB transport");
    }
}
