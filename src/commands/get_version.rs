use crate::apdu::Instruction;
use crate::error::LedgerError;
use crate::protocol;
use crate::transport::Transport;
use crate::types::AppVersion;

/// Response: `[major][minor][patch][app_name...]`
pub fn exec(transport: &dyn Transport) -> Result<AppVersion, LedgerError> {
    let result = protocol::execute(transport, Instruction::GetVersion, &[])?;
    parse_version_response(&result)
}

pub(crate) fn parse_version_response(data: &[u8]) -> Result<AppVersion, LedgerError> {
    if data.len() < 4 {
        return Err(LedgerError::InvalidResponse(
            "version response too short - is the IOTA app running?".into(),
        ));
    }

    let name = String::from_utf8_lossy(&data[3..]).to_string();

    Ok(AppVersion {
        major: data[0],
        minor: data[1],
        patch: data[2],
        name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_version() {
        let data = [0x01, 0x02, 0x03, b'i', b'o', b't', b'a'];
        let v = parse_version_response(&data).unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.name, "iota");
    }

    #[test]
    fn parse_minimum_valid_response() {
        // 3 version bytes + 1 name byte (minimum 4 bytes)
        let data = [0x00, 0x09, 0x00, b'x'];
        let v = parse_version_response(&data).unwrap();
        assert_eq!((v.major, v.minor, v.patch), (0, 9, 0));
        assert_eq!(v.name, "x");
    }

    #[test]
    fn parse_too_short_response() {
        for len in 0..4 {
            let data = vec![0x01; len];
            let err = parse_version_response(&data).unwrap_err();
            assert!(matches!(err, LedgerError::InvalidResponse(_)));
        }
    }

    #[test]
    fn parse_invalid_utf8_uses_lossy() {
        let data = [0x01, 0x00, 0x00, 0xFF, 0xFE];
        let v = parse_version_response(&data).unwrap();
        // from_utf8_lossy replaces invalid bytes with U+FFFD
        assert!(v.name.contains('\u{FFFD}'));
    }
}
