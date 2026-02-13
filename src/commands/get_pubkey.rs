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
