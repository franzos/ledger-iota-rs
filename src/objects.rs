//! Object data encoding for clear signing.
//!
//! For non-standard token transfers, the host provides object data so the
//! device can show coin details instead of falling back to blind signing.

use byteorder::{LittleEndian, WriteBytesExt};

/// Provides coin details so the device can clear-sign non-standard tokens.
#[derive(Debug, Clone)]
pub struct ObjectData {
    pub data: MoveObject,
    pub owner: Owner,
    pub previous_transaction: [u8; 33],
    pub storage_rebate: u64,
}

#[derive(Debug, Clone)]
pub struct MoveObject {
    pub type_: MoveObjectType,
    pub has_public_transfer: bool,
    pub version: u64,
    pub contents: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum MoveObjectType {
    GasCoin,
    StakedIota,
    Coin(TypeTag),
}

#[derive(Debug, Clone)]
pub struct TypeTag {
    pub address: [u8; 32],
    pub module: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum Owner {
    AddressOwner([u8; 32]),
    ObjectOwner([u8; 32]),
    Shared { initial_shared_version: u64 },
    Immutable,
}

impl ObjectData {
    pub fn gas_coin(
        version: u64,
        contents: Vec<u8>,
        owner: Owner,
        previous_transaction: [u8; 33],
        storage_rebate: u64,
    ) -> Self {
        Self {
            data: MoveObject {
                type_: MoveObjectType::GasCoin,
                has_public_transfer: true,
                version,
                contents,
            },
            owner,
            previous_transaction,
            storage_rebate,
        }
    }

    pub fn coin(
        type_tag: TypeTag,
        version: u64,
        contents: Vec<u8>,
        owner: Owner,
        previous_transaction: [u8; 33],
        storage_rebate: u64,
    ) -> Self {
        Self {
            data: MoveObject {
                type_: MoveObjectType::Coin(type_tag),
                has_public_transfer: true,
                version,
                contents,
            },
            owner,
            previous_transaction,
            storage_rebate,
        }
    }

    pub fn staked_iota(
        version: u64,
        contents: Vec<u8>,
        owner: Owner,
        previous_transaction: [u8; 33],
        storage_rebate: u64,
    ) -> Self {
        Self {
            data: MoveObject {
                type_: MoveObjectType::StakedIota,
                has_public_transfer: false,
                version,
                contents,
            },
            owner,
            previous_transaction,
            storage_rebate,
        }
    }

    fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        buf.push(0x00); // ObjectData::Move
        match &self.data.type_ {
            MoveObjectType::GasCoin => buf.push(1),
            MoveObjectType::StakedIota => buf.push(2),
            MoveObjectType::Coin(tag) => {
                buf.push(3);
                encode_type_tag(&mut buf, tag);
            }
        }

        buf.push(self.data.has_public_transfer as u8);
        buf.write_u64::<LittleEndian>(self.data.version).unwrap();

        // BCS Vec<u8>: ULEB128 length prefix
        write_uleb128(&mut buf, self.data.contents.len() as u64);
        buf.extend_from_slice(&self.data.contents);

        match &self.owner {
            Owner::AddressOwner(addr) => {
                buf.push(0);
                buf.extend_from_slice(addr);
            }
            Owner::ObjectOwner(addr) => {
                buf.push(1);
                buf.extend_from_slice(addr);
            }
            Owner::Shared {
                initial_shared_version,
            } => {
                buf.push(2);
                buf.write_u64::<LittleEndian>(*initial_shared_version)
                    .unwrap();
            }
            Owner::Immutable => {
                buf.push(3);
            }
        }

        buf.extend_from_slice(&self.previous_transaction);
        buf.write_u64::<LittleEndian>(self.storage_rebate).unwrap();

        buf
    }
}

fn encode_type_tag(buf: &mut Vec<u8>, tag: &TypeTag) {
    buf.extend_from_slice(&tag.address);
    write_bcs_string(buf, &tag.module);
    write_bcs_string(buf, &tag.name);
    write_uleb128(buf, 0); // no type_params
}

fn write_bcs_string(buf: &mut Vec<u8>, s: &str) {
    write_uleb128(buf, s.len() as u64);
    buf.extend_from_slice(s.as_bytes());
}

fn write_uleb128(buf: &mut Vec<u8>, mut val: u64) {
    loop {
        let mut byte = (val & 0x7F) as u8;
        val >>= 7;
        if val != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if val == 0 {
            break;
        }
    }
}

/// Wire format for SignTx parameter 3:
/// `[count: u32 LE][obj_len: u32 LE][obj_data]...`
pub fn encode_objects(objects: &[ObjectData]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.write_u32::<LittleEndian>(objects.len() as u32).unwrap();

    for obj in objects {
        let encoded = obj.encode();
        buf.write_u32::<LittleEndian>(encoded.len() as u32).unwrap();
        buf.extend_from_slice(&encoded);
    }

    buf
}

#[cfg(feature = "iota-sdk-types")]
impl TryFrom<iota_sdk_types::Object> for ObjectData {
    type Error = String;

    fn try_from(obj: iota_sdk_types::Object) -> Result<Self, Self::Error> {
        let move_struct = match obj.data {
            iota_sdk_types::ObjectData::Struct(s) => s,
            iota_sdk_types::ObjectData::Package(_) => {
                return Err("packages cannot be converted to ObjectData for clear signing".into());
            }
        };

        let (type_, has_public_transfer) = convert_struct_type(&move_struct.type_)?;

        // BCS-encode the digest: 1-byte length prefix (0x20 = 32) + 32 bytes
        let mut prev_tx = [0u8; 33];
        prev_tx[0] = 32;
        prev_tx[1..].copy_from_slice(obj.previous_transaction.inner());

        Ok(ObjectData {
            data: MoveObject {
                type_,
                has_public_transfer,
                version: move_struct.version,
                contents: move_struct.contents,
            },
            owner: convert_owner(obj.owner),
            previous_transaction: prev_tx,
            storage_rebate: obj.storage_rebate,
        })
    }
}

#[cfg(feature = "iota-sdk-types")]
fn convert_struct_type(tag: &iota_sdk_types::StructTag) -> Result<(MoveObjectType, bool), String> {
    use iota_sdk_types::Address as SdkAddr;

    if let Some(coin_type) = tag.coin_type_opt() {
        // Check if it's the native IOTA coin (GasCoin)
        if let iota_sdk_types::TypeTag::Struct(inner) = coin_type {
            if inner.address == SdkAddr::FRAMEWORK
                && inner.module.as_str() == "iota"
                && inner.name.as_str() == "IOTA"
                && inner.type_params.is_empty()
            {
                return Ok((MoveObjectType::GasCoin, true));
            }
        }

        // Non-IOTA coin — extract the inner type tag
        let inner_tag = match coin_type {
            iota_sdk_types::TypeTag::Struct(s) => TypeTag {
                address: s.address.into_inner(),
                module: s.module.as_str().to_string(),
                name: s.name.as_str().to_string(),
            },
            _ => return Err("coin type parameter must be a struct type".into()),
        };
        Ok((MoveObjectType::Coin(inner_tag), true))
    } else if tag.address == SdkAddr::SYSTEM
        && tag.module.as_str() == "staking_pool"
        && tag.name.as_str() == "StakedIota"
        && tag.type_params.is_empty()
    {
        Ok((MoveObjectType::StakedIota, false))
    } else {
        Err("unsupported object type for clear signing (expected coin or staked IOTA)".into())
    }
}

#[cfg(feature = "iota-sdk-types")]
fn convert_owner(owner: iota_sdk_types::Owner) -> Owner {
    match owner {
        iota_sdk_types::Owner::Address(addr) => Owner::AddressOwner(addr.into_inner()),
        iota_sdk_types::Owner::Object(id) => Owner::ObjectOwner(id.into_inner()),
        iota_sdk_types::Owner::Shared(version) => Owner::Shared {
            initial_shared_version: version,
        },
        iota_sdk_types::Owner::Immutable => Owner::Immutable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_gas_coin_object() {
        let obj = ObjectData::gas_coin(
            42,
            vec![0u8; 40],
            Owner::AddressOwner([0xAA; 32]),
            [0u8; 33],
            1000,
        );
        let encoded = obj.encode();
        assert_eq!(encoded[0], 0x00);
        assert_eq!(encoded[1], 1);
        assert_eq!(encoded[2], 1);
    }

    #[test]
    fn encode_objects_format() {
        let obj = ObjectData::gas_coin(1, vec![0u8; 40], Owner::Immutable, [0u8; 33], 0);
        let buf = encode_objects(&[obj]);

        assert_eq!(&buf[0..4], &[1, 0, 0, 0]); // count=1 LE
        let obj_len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
        assert_eq!(buf.len(), 4 + 4 + obj_len);
    }

    #[test]
    fn uleb128_encoding() {
        let mut buf = Vec::new();
        write_uleb128(&mut buf, 0);
        assert_eq!(buf, vec![0]);

        buf.clear();
        write_uleb128(&mut buf, 127);
        assert_eq!(buf, vec![127]);

        buf.clear();
        write_uleb128(&mut buf, 128);
        assert_eq!(buf, vec![0x80, 0x01]);

        buf.clear();
        write_uleb128(&mut buf, 300);
        assert_eq!(buf, vec![0xAC, 0x02]);
    }
}
