use ledger_iota::{Bip32Path, LedgerIota, TransportType};

fn main() {
    #[cfg(feature = "hid")]
    {
        let ledger = LedgerIota::new(&TransportType::NativeHID).unwrap_or_else(|e| {
            eprintln!("failed to connect: {e}");
            std::process::exit(1);
        });
        let path = Bip32Path::iota(0, 0, 0);
        let message = b"Hi, this is my wallet";

        match ledger.sign_message(message, &path) {
            Ok(sig) => println!("signature: {sig}"),
            Err(e) => eprintln!("signing failed: {e}"),
        }
    }
    #[cfg(not(feature = "hid"))]
    {
        eprintln!("enable the 'hid' feature to use USB transport");
    }
}
