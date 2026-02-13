use crate::apdu::Instruction;
use crate::error::LedgerError;
use crate::protocol;
use crate::transport::Transport;
use crate::types::{Address, Bip32Path, PublicKey};

/// Same wire format as GetPubkey but triggers on-device confirmation.
/// Will block until the user approves or rejects.
pub fn exec(
    transport: &dyn Transport,
    path: &Bip32Path,
) -> Result<(PublicKey, Address), LedgerError> {
    let param = path.serialize();
    let result = protocol::execute(transport, Instruction::VerifyAddress, &[param])?;
    super::get_pubkey::parse_pubkey_response(&result)
}
