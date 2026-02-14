use crate::apdu::Instruction;
use crate::error::LedgerError;
use crate::protocol;
use crate::transport::Transport;
use crate::types::{Address, Bip32Path, PublicKey};

/// Response: `[pubkey_len][pubkey (32)][address_len][address (32)]`
pub fn exec(
    transport: &dyn Transport,
    path: &Bip32Path,
) -> Result<(PublicKey, Address), LedgerError> {
    let param = path.serialize();
    let result = protocol::execute(transport, Instruction::GetPubkey, &[param])?;
    parse_pubkey_response(&result)
}

pub(crate) fn parse_pubkey_response(data: &[u8]) -> Result<(PublicKey, Address), LedgerError> {
    if data.is_empty() {
        return Err(LedgerError::InvalidResponse("empty pubkey response".into()));
    }

    let pk_len = data[0] as usize;
    if pk_len != 32 || data.len() < 1 + pk_len + 1 {
        return Err(LedgerError::InvalidResponse(format!(
            "unexpected pubkey length: {pk_len}"
        )));
    }

    let mut pubkey = [0u8; 32];
    pubkey.copy_from_slice(&data[1..33]);

    let addr_len = data[33] as usize;
    if addr_len != 32 || data.len() < 34 + addr_len {
        return Err(LedgerError::InvalidResponse(format!(
            "unexpected address length: {addr_len}"
        )));
    }

    let mut address = [0u8; 32];
    address.copy_from_slice(&data[34..66]);

    Ok((PublicKey(pubkey), Address(address)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_response() -> Vec<u8> {
        let mut data = Vec::new();
        data.push(32); // pk_len
        data.extend_from_slice(&[0xAA; 32]); // pubkey
        data.push(32); // addr_len
        data.extend_from_slice(&[0xBB; 32]); // address
        data
    }

    #[test]
    fn parse_valid_response() {
        let (pk, addr) = parse_pubkey_response(&valid_response()).unwrap();
        assert_eq!(pk.0, [0xAA; 32]);
        assert_eq!(addr.0, [0xBB; 32]);
    }

    #[test]
    fn parse_empty_response() {
        let err = parse_pubkey_response(&[]).unwrap_err();
        assert!(matches!(err, LedgerError::InvalidResponse(_)));
    }

    #[test]
    fn parse_wrong_pubkey_length() {
        let mut data = vec![31]; // pk_len = 31
        data.extend_from_slice(&[0; 64]);
        let err = parse_pubkey_response(&data).unwrap_err();
        assert!(matches!(err, LedgerError::InvalidResponse(_)));
    }

    #[test]
    fn parse_truncated_pubkey() {
        let mut data = vec![32]; // pk_len = 32
        data.extend_from_slice(&[0; 10]); // only 10 bytes of pubkey
        let err = parse_pubkey_response(&data).unwrap_err();
        assert!(matches!(err, LedgerError::InvalidResponse(_)));
    }

    #[test]
    fn parse_wrong_address_length() {
        let mut data = Vec::new();
        data.push(32);
        data.extend_from_slice(&[0; 32]); // valid pubkey
        data.push(31); // addr_len = 31
        data.extend_from_slice(&[0; 64]);
        let err = parse_pubkey_response(&data).unwrap_err();
        assert!(matches!(err, LedgerError::InvalidResponse(_)));
    }

    #[test]
    fn parse_truncated_address() {
        let mut data = Vec::new();
        data.push(32);
        data.extend_from_slice(&[0; 32]); // valid pubkey
        data.push(32); // addr_len = 32
        data.extend_from_slice(&[0; 10]); // only 10 bytes of address
        let err = parse_pubkey_response(&data).unwrap_err();
        assert!(matches!(err, LedgerError::InvalidResponse(_)));
    }
}
