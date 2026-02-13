use crate::apdu::Instruction;
use crate::error::LedgerError;
use crate::protocol;
use crate::transport::Transport;

/// The app exits before it can send a proper response, so we
/// ignore transport/protocol errors here.
pub fn exec(transport: &dyn Transport) -> Result<(), LedgerError> {
    let _ = protocol::execute(transport, Instruction::Quit, &[]);
    Ok(())
}
