use crate::apdu::Instruction;
use crate::error::LedgerError;
use crate::protocol;
use crate::transport::Transport;
use crate::types::AppVersion;

/// Response: `[major][minor][patch][app_name...]`
pub fn exec(transport: &dyn Transport) -> Result<AppVersion, LedgerError> {
    let result = protocol::execute(transport, Instruction::GetVersion, &[])?;

    if result.len() < 4 {
        return Err(LedgerError::InvalidResponse(
            "version response too short - is the IOTA app running?".into(),
        ));
    }

    let name = String::from_utf8_lossy(&result[3..]).to_string();

    Ok(AppVersion {
        major: result[0],
        minor: result[1],
        patch: result[2],
        name,
    })
}
