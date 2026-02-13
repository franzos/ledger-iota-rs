//! Transaction building helpers for IOTA Rebased.
//!
//! Provides BCS-encoded transaction construction so callers don't have to
//! hand-roll the binary format.

/// Reference to a gas coin object (from RPC).
#[derive(Debug, Clone)]
pub struct GasCoinRef {
    pub object_id: [u8; 32],
    pub version: u64,
    pub digest: [u8; 32],
}

/// BCS-encode an `IntentMessage<TransactionData::V1>` that splits `amount`
/// nanos from the gas coin and transfers them to `recipient`.
///
/// The returned bytes include the intent prefix `[0, 0, 0]` and are ready
/// to be passed directly to [`LedgerIota::sign_tx`](crate::LedgerIota::sign_tx).
///
/// # ProgrammableTransaction layout
///
/// - inputs:   `[Pure(recipient), Pure(amount)]`
/// - commands: `[SplitCoins(GasCoin, [Input(1)]), TransferObjects([Result(0)], Input(0))]`
#[must_use]
pub fn build_transfer_tx(
    sender: &[u8; 32],
    recipient: &[u8; 32],
    amount: u64,
    gas: &GasCoinRef,
    gas_budget: u64,
    gas_price: u64,
) -> Vec<u8> {
    let mut tx = Vec::new();

    // IntentMessage prefix: version=0, scope=0 (TransactionData), app_id=0 (IOTA)
    tx.extend_from_slice(&[0x00, 0x00, 0x00]);

    // TransactionData::V1
    tx.push(0x00);
    // TransactionKind::ProgrammableTransaction
    tx.push(0x00);

    // --- inputs: Vec<CallArg> (length=2) ---
    tx.push(0x02);
    //   [0] Pure(recipient)
    tx.push(0x00); // Pure variant
    tx.push(32); // ULEB128 vec length
    tx.extend_from_slice(recipient);
    //   [1] Pure(amount as u64 LE)
    tx.push(0x00);
    tx.push(8);
    tx.extend_from_slice(&amount.to_le_bytes());

    // --- commands: Vec<Command> (length=2) ---
    tx.push(0x02);
    //   [0] SplitCoins(GasCoin, [Input(1)])
    tx.push(0x02); // SplitCoins
    tx.push(0x00); // Argument::GasCoin
    tx.push(0x01); // vec len=1
    tx.push(0x01); // Argument::Input
    tx.extend_from_slice(&1u16.to_le_bytes());
    //   [1] TransferObjects([Result(0)], Input(0))
    tx.push(0x01); // TransferObjects
    tx.push(0x01); // vec len=1
    tx.push(0x02); // Argument::Result
    tx.extend_from_slice(&0u16.to_le_bytes());
    tx.push(0x01); // Argument::Input
    tx.extend_from_slice(&0u16.to_le_bytes());

    // --- sender ---
    tx.extend_from_slice(sender);

    // --- GasData ---
    // payment: Vec<ObjectRef> (length=1)
    tx.push(0x01);
    tx.extend_from_slice(&gas.object_id); // ObjectID
    tx.extend_from_slice(&gas.version.to_le_bytes()); // SequenceNumber
    tx.push(32); // BCS Digest length prefix
    tx.extend_from_slice(&gas.digest); // ObjectDigest (32 bytes)
                                       // owner
    tx.extend_from_slice(sender);
    // price
    tx.extend_from_slice(&gas_price.to_le_bytes());
    // budget
    tx.extend_from_slice(&gas_budget.to_le_bytes());

    // TransactionExpiration::None
    tx.push(0x00);

    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_tx_has_intent_prefix() {
        let sender = [0x01; 32];
        let recipient = [0x02; 32];
        let gas = GasCoinRef {
            object_id: [0u8; 32],
            version: 1,
            digest: [0u8; 32],
        };

        let tx = build_transfer_tx(&sender, &recipient, 1_000_000, &gas, 10_000_000, 1000);

        // intent prefix
        assert_eq!(&tx[0..3], &[0, 0, 0]);
        // TransactionData::V1
        assert_eq!(tx[3], 0x00);
        // TransactionKind::ProgrammableTransaction
        assert_eq!(tx[4], 0x00);
    }

    #[test]
    fn transfer_tx_deterministic() {
        let sender = [0xAA; 32];
        let recipient = [0xBB; 32];
        let gas = GasCoinRef {
            object_id: [0xCC; 32],
            version: 42,
            digest: [0xDD; 32],
        };

        let a = build_transfer_tx(&sender, &recipient, 500, &gas, 5_000_000, 750);
        let b = build_transfer_tx(&sender, &recipient, 500, &gas, 5_000_000, 750);
        assert_eq!(a, b);
    }
}
