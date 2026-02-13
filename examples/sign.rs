use ledger_iota::{Bip32Path, LedgerIota, TransportType};

fn main() {
    #[cfg(feature = "hid")]
    {
        let ledger = LedgerIota::new(&TransportType::NativeHID).unwrap_or_else(|e| {
            eprintln!("failed to connect: {e}");
            std::process::exit(1);
        });
        let path = Bip32Path::iota(0, 0, 0);

        // dummy tx — swap in real BCS-encoded bytes
        let tx = vec![0u8; 64];

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
