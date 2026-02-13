use sha2::{Digest, Sha256};

const BLOCK_DATA_SIZE: usize = 180;

/// Up to 180 bytes of data + a 32-byte SHA256 hash pointing to the
/// next block (all zeros for the last one).
#[derive(Debug, Clone)]
pub struct Block {
    pub next_hash: [u8; 32],
    pub data: Vec<u8>,
}

impl Block {
    pub fn serialized_len(&self) -> usize {
        32 + self.data.len()
    }

    pub fn serialize_into(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.next_hash);
        buf.extend_from_slice(&self.data);
    }

    pub(crate) fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.serialized_len());
        self.serialize_into(&mut buf);
        buf
    }
}

/// Split data into SHA256-linked blocks. Built backwards so each
/// block's `next_hash` points to the serialized hash of the next one
/// (last block gets all zeros).
pub fn build_block_chain(data: &[u8]) -> Vec<Block> {
    if data.is_empty() {
        return vec![Block {
            next_hash: [0u8; 32],
            data: Vec::new(),
        }];
    }

    let chunks: Vec<&[u8]> = data.chunks(BLOCK_DATA_SIZE).collect();
    let mut blocks: Vec<Block> = Vec::with_capacity(chunks.len());

    let mut next_hash = [0u8; 32];

    for chunk in chunks.iter().rev() {
        let block = Block {
            next_hash,
            data: chunk.to_vec(),
        };
        next_hash = hash_block(&block);
        blocks.push(block);
    }

    blocks.reverse();
    blocks
}

/// SHA256 of the serialized block (`next_hash ++ data`).
pub fn hash_block(block: &Block) -> [u8; 32] {
    sha256(&block.serialize())
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_small_block() {
        let data = b"hello";
        let blocks = build_block_chain(data);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].data, b"hello");
        assert_eq!(blocks[0].next_hash, [0u8; 32]);
    }

    #[test]
    fn empty_data() {
        let blocks = build_block_chain(b"");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].data.is_empty());
        assert_eq!(blocks[0].next_hash, [0u8; 32]);
    }

    #[test]
    fn multiple_blocks() {
        let data = vec![0xAB; 400]; // 180 + 180 + 40
        let blocks = build_block_chain(&data);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].data.len(), 180);
        assert_eq!(blocks[1].data.len(), 180);
        assert_eq!(blocks[2].data.len(), 40);

        assert_eq!(blocks[2].next_hash, [0u8; 32]);
        assert_eq!(blocks[1].next_hash, hash_block(&blocks[2]));
        assert_eq!(blocks[0].next_hash, hash_block(&blocks[1]));
    }

    #[test]
    fn exact_block_boundary() {
        let data = vec![0xCD; 180];
        let blocks = build_block_chain(&data);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].data.len(), 180);
    }

    #[test]
    fn hash_chain_integrity() {
        let data = vec![0xFF; 500];
        let blocks = build_block_chain(&data);

        for i in 0..blocks.len() - 1 {
            let expected = hash_block(&blocks[i + 1]);
            assert_eq!(blocks[i].next_hash, expected);
        }
    }
}
