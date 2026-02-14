//! APDU command and response types.
//!
//! The IOTA app uses CLA `0x00` for all commands with P1/P2 always `0x00`.

#[derive(Debug, Clone)]
pub struct ApduCommand {
    pub cla: u8,
    pub ins: u8,
    pub p1: u8,
    pub p2: u8,
    pub data: Vec<u8>,
}

impl ApduCommand {
    pub fn new(ins: u8) -> Self {
        Self {
            cla: 0x00,
            ins,
            p1: 0x00,
            p2: 0x00,
            data: Vec::new(),
        }
    }

    pub fn with_data(ins: u8, data: Vec<u8>) -> Self {
        Self {
            cla: 0x00,
            ins,
            p1: 0x00,
            p2: 0x00,
            data,
        }
    }

    /// Wire format: `[CLA][INS][P1][P2][LC][DATA]`
    ///
    /// # Panics
    ///
    /// Panics if `data` exceeds 255 bytes (short APDU LC limit).
    pub fn serialize(&self) -> Vec<u8> {
        assert!(
            self.data.len() <= 255,
            "APDU data too long: {} bytes (max 255)",
            self.data.len()
        );
        let mut buf = Vec::with_capacity(5 + self.data.len());
        buf.push(self.cla);
        buf.push(self.ins);
        buf.push(self.p1);
        buf.push(self.p2);
        buf.push(self.data.len() as u8);
        buf.extend_from_slice(&self.data);
        buf
    }
}

/// APDU response - last 2 bytes are the status word, everything before
/// that is the payload. Use [`data()`](ApduAnswer::data) to strip the SW.
#[derive(Debug, Clone)]
pub struct ApduAnswer {
    raw: Vec<u8>,
}

impl ApduAnswer {
    pub fn from_raw(raw: Vec<u8>) -> Self {
        Self { raw }
    }

    pub fn retcode(&self) -> u16 {
        if self.raw.len() < 2 {
            return 0;
        }
        let len = self.raw.len();
        ((self.raw[len - 2] as u16) << 8) | (self.raw[len - 1] as u16)
    }

    /// Payload only - strips the trailing 2-byte status word.
    pub fn data(&self) -> &[u8] {
        if self.raw.len() < 2 {
            return &[];
        }
        &self.raw[..self.raw.len() - 2]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- ApduCommand --

    #[test]
    fn serialize_empty_data() {
        let cmd = ApduCommand::new(0x42);
        let buf = cmd.serialize();
        assert_eq!(buf, vec![0x00, 0x42, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn serialize_with_data() {
        let cmd = ApduCommand::with_data(0x01, vec![0xAA, 0xBB]);
        let buf = cmd.serialize();
        assert_eq!(buf, vec![0x00, 0x01, 0x00, 0x00, 0x02, 0xAA, 0xBB]);
    }

    #[test]
    fn serialize_max_255_bytes() {
        let cmd = ApduCommand::with_data(0x01, vec![0xFF; 255]);
        let buf = cmd.serialize();
        assert_eq!(buf.len(), 5 + 255);
        assert_eq!(buf[4], 255); // LC byte
    }

    #[test]
    #[should_panic(expected = "APDU data too long")]
    fn serialize_panics_at_256_bytes() {
        let cmd = ApduCommand::with_data(0x01, vec![0x00; 256]);
        cmd.serialize();
    }

    // -- ApduAnswer --

    #[test]
    fn retcode_empty_response() {
        let ans = ApduAnswer::from_raw(vec![]);
        assert_eq!(ans.retcode(), 0);
    }

    #[test]
    fn retcode_single_byte() {
        let ans = ApduAnswer::from_raw(vec![0x90]);
        assert_eq!(ans.retcode(), 0);
    }

    #[test]
    fn retcode_just_status_word() {
        let ans = ApduAnswer::from_raw(vec![0x90, 0x00]);
        assert_eq!(ans.retcode(), 0x9000);
    }

    #[test]
    fn retcode_with_payload() {
        let ans = ApduAnswer::from_raw(vec![0x01, 0x02, 0x03, 0x69, 0x85]);
        assert_eq!(ans.retcode(), 0x6985);
    }

    #[test]
    fn data_empty_response() {
        let ans = ApduAnswer::from_raw(vec![]);
        assert!(ans.data().is_empty());
    }

    #[test]
    fn data_single_byte() {
        let ans = ApduAnswer::from_raw(vec![0x90]);
        assert!(ans.data().is_empty());
    }

    #[test]
    fn data_just_status_word() {
        let ans = ApduAnswer::from_raw(vec![0x90, 0x00]);
        assert!(ans.data().is_empty());
    }

    #[test]
    fn data_strips_status_word() {
        let ans = ApduAnswer::from_raw(vec![0xAA, 0xBB, 0xCC, 0x90, 0x00]);
        assert_eq!(ans.data(), &[0xAA, 0xBB, 0xCC]);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Instruction {
    GetVersion = 0x00,
    VerifyAddress = 0x01,
    GetPubkey = 0x02,
    SignTx = 0x03,
    Quit = 0xFF,
}
