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
    parse_signature(&result)
}

pub(crate) fn parse_signature(data: &[u8]) -> Result<Signature, LedgerError> {
    if data.len() < 64 {
        return Err(LedgerError::InvalidResponse(format!(
            "expected 64-byte signature, got {} bytes",
            data.len()
        )));
    }

    let mut sig = [0u8; 64];
    sig.copy_from_slice(&data[..64]);
    Ok(Signature(sig))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_64_byte_signature() {
        let data = [0xAB; 64];
        let sig = parse_signature(&data).unwrap();
        assert_eq!(sig.0, [0xAB; 64]);
    }

    #[test]
    fn parse_longer_response_uses_first_64() {
        let mut data = vec![0xCD; 80];
        data[0] = 0x01;
        let sig = parse_signature(&data).unwrap();
        assert_eq!(sig.0[0], 0x01);
        assert_eq!(sig.0[63], 0xCD);
    }

    #[test]
    fn parse_too_short_signature() {
        let data = [0x00; 63];
        let err = parse_signature(&data).unwrap_err();
        assert!(matches!(err, LedgerError::InvalidResponse(_)));
    }

    #[test]
    fn parse_empty_signature() {
        let err = parse_signature(&[]).unwrap_err();
        assert!(matches!(err, LedgerError::InvalidResponse(_)));
    }
}
