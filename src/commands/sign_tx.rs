use crate::apdu::Instruction;
use crate::error::LedgerError;
use crate::protocol;
use crate::transport::Transport;
use crate::types::{Bip32Path, Signature};

/// Params: (1) `[tx_size: u32 LE][tx_bytes]`, (2) BIP32 path,
/// (3, optional) encoded objects for clear signing.
/// Returns a 64-byte Ed25519 signature.
pub fn exec(
    transport: &dyn Transport,
    tx: &[u8],
    path: &Bip32Path,
    objects: Option<&[u8]>,
) -> Result<Signature, LedgerError> {
    let mut param1 = Vec::with_capacity(4 + tx.len());
    param1.extend_from_slice(&(tx.len() as u32).to_le_bytes());
    param1.extend_from_slice(tx);

    let param2 = path.serialize();
    let mut params = vec![param1, param2];

    if let Some(obj_data) = objects {
        params.push(obj_data.to_vec());
    }

    let result = protocol::execute(transport, Instruction::SignTx, &params)?;

    if result.len() < 64 {
        return Err(LedgerError::InvalidResponse(format!(
            "expected 64-byte signature, got {} bytes",
            result.len()
        )));
    }

    let mut sig = [0u8; 64];
    sig.copy_from_slice(&result[..64]);
    Ok(Signature(sig))
}
