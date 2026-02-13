use std::sync::Mutex;

use crate::apdu::{ApduAnswer, ApduCommand};
use crate::error::TransportError;
use crate::transport::Transport;

const LEDGER_VID: u16 = 0x2c97;
const LEDGER_USAGE_PAGE: u16 = 0xFFA0;
const LEDGER_CHANNEL: u16 = 0x0101;
const LEDGER_TAG: u8 = 0x05;
const LEDGER_PACKET_WRITE_SIZE: usize = 65;
const LEDGER_PACKET_READ_SIZE: usize = 64;
const LEDGER_TIMEOUT_MS: i32 = 30_000;
const CHUNK_SIZE: usize = LEDGER_PACKET_WRITE_SIZE - 6;

/// Detected from the upper byte of the USB product ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    NanoS,
    NanoSPlus,
    NanoX,
    Flex,
    Stax,
    Unknown(u16),
}

impl DeviceType {
    /// Upper byte of the PID encodes the device family:
    /// `0x10` = Nano S, `0x40` = Nano X, `0x50` = Nano S+,
    /// `0x60` = Stax, `0x70` = Flex.
    pub fn from_product_id(pid: u16) -> Self {
        match pid >> 8 {
            0x10 => Self::NanoS,
            0x40 => Self::NanoX,
            0x50 => Self::NanoSPlus,
            0x60 => Self::Stax,
            0x70 => Self::Flex,
            _ => Self::Unknown(pid),
        }
    }
}

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NanoS => write!(f, "Nano S"),
            Self::NanoSPlus => write!(f, "Nano S+"),
            Self::NanoX => write!(f, "Nano X"),
            Self::Flex => write!(f, "Flex"),
            Self::Stax => write!(f, "Stax"),
            Self::Unknown(pid) => write!(f, "Unknown (0x{pid:04X})"),
        }
    }
}

pub struct HidTransport {
    device: Mutex<hidapi::HidDevice>,
    device_type: DeviceType,
}

impl HidTransport {
    pub fn new() -> Result<Self, TransportError> {
        let api = hidapi::HidApi::new().map_err(|e| TransportError::Comm(e.to_string()))?;

        for info in api.device_list() {
            if info.vendor_id() == LEDGER_VID && info.usage_page() == LEDGER_USAGE_PAGE {
                let device_type = DeviceType::from_product_id(info.product_id());
                let device = info
                    .open_device(&api)
                    .map_err(|e| TransportError::Comm(e.to_string()))?;
                log::info!("connected to Ledger {device_type}");
                return Ok(Self {
                    device: Mutex::new(device),
                    device_type,
                });
            }
        }

        Err(TransportError::DeviceNotFound)
    }

    pub fn device_type(&self) -> DeviceType {
        self.device_type
    }

    fn write_apdu(device: &hidapi::HidDevice, apdu: &[u8]) -> Result<(), TransportError> {
        // HID framing: 2-byte length prefix, then APDU, split into 59-byte chunks
        let mut payload = Vec::with_capacity(2 + apdu.len());
        payload.push(((apdu.len() >> 8) & 0xFF) as u8);
        payload.push((apdu.len() & 0xFF) as u8);
        payload.extend_from_slice(apdu);

        let mut buffer = vec![0u8; LEDGER_PACKET_WRITE_SIZE];

        for (seq_idx, chunk) in payload.chunks(CHUNK_SIZE).enumerate() {
            buffer[0] = 0x00;
            buffer[1] = ((LEDGER_CHANNEL >> 8) & 0xFF) as u8;
            buffer[2] = (LEDGER_CHANNEL & 0xFF) as u8;
            buffer[3] = LEDGER_TAG;
            buffer[4] = ((seq_idx >> 8) & 0xFF) as u8;
            buffer[5] = (seq_idx & 0xFF) as u8;

            buffer[6..].fill(0);
            buffer[6..6 + chunk.len()].copy_from_slice(chunk);

            device
                .write(&buffer)
                .map_err(|e| TransportError::Comm(e.to_string()))?;
        }

        Ok(())
    }

    fn read_apdu(device: &hidapi::HidDevice) -> Result<Vec<u8>, TransportError> {
        let mut buffer = vec![0u8; LEDGER_PACKET_READ_SIZE];
        let mut result = Vec::new();
        let mut expected_len: Option<usize> = None;
        let mut seq_idx: u16 = 0;

        loop {
            let n = device
                .read_timeout(&mut buffer, LEDGER_TIMEOUT_MS)
                .map_err(|e| TransportError::Comm(e.to_string()))?;

            if n == 0 {
                return Err(TransportError::Timeout(LEDGER_TIMEOUT_MS as u32));
            }

            let channel = ((buffer[0] as u16) << 8) | (buffer[1] as u16);
            if channel != LEDGER_CHANNEL {
                return Err(TransportError::Comm("HID channel mismatch".into()));
            }
            if buffer[2] != LEDGER_TAG {
                return Err(TransportError::Comm("HID tag mismatch".into()));
            }

            let pkt_seq = ((buffer[3] as u16) << 8) | (buffer[4] as u16);
            if pkt_seq != seq_idx {
                return Err(TransportError::Comm(format!(
                    "sequence mismatch: expected {seq_idx}, got {pkt_seq}"
                )));
            }

            let data_start;
            if seq_idx == 0 {
                // First packet has a 2-byte length prefix before the data
                let apdu_len = ((buffer[5] as usize) << 8) | (buffer[6] as usize);
                expected_len = Some(apdu_len);
                data_start = 7;
            } else {
                data_start = 5;
            }

            if n < data_start {
                return Err(TransportError::Comm(format!(
                    "HID short read: got {n} bytes, need at least {data_start}"
                )));
            }

            let remaining = expected_len.unwrap() - result.len();
            let available = n - data_start;
            let take = remaining.min(available);
            result.extend_from_slice(&buffer[data_start..data_start + take]);

            if result.len() >= expected_len.unwrap() {
                break;
            }

            seq_idx += 1;
        }

        Ok(result)
    }
}

impl Transport for HidTransport {
    fn exchange(&self, command: &ApduCommand) -> Result<ApduAnswer, TransportError> {
        let device = self
            .device
            .lock()
            .map_err(|e| TransportError::Comm(format!("mutex poisoned: {e}")))?;
        let serialized = command.serialize();
        Self::write_apdu(&device, &serialized)?;
        let response = Self::read_apdu(&device)?;
        Ok(ApduAnswer::from_raw(response))
    }
}
