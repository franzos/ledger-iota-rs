# Ledger Library for IOTA Application

Rust client library for talking to the IOTA Rebased Ledger app (`app-iota` v1.0.x). Standalone — no IOTA SDK dependency. It handles USB HID for real devices and TCP for the Speculos simulator.

See the [IOTA Ledger guide](https://docs.iota.org/users/iota-wallet/how-to/import/ledger) for device setup.

## Usage

### Connect and derive address

```rust
use ledger_iota::{LedgerIota, Bip32Path, TransportType};

let ledger = LedgerIota::new(&TransportType::NativeHID)?;

let version = ledger.get_version()?;
println!("{version}");

let path = Bip32Path::iota(0, 0, 0);
let (pubkey, address) = ledger.get_pubkey(&path)?;
println!("address: {address}");
```

### Verify address on device

Prompts the user to confirm the address on the Ledger display:

```rust
let (pubkey, address) = ledger.verify_address(&path)?;
```

### Build and sign a transfer

```rust
use ledger_iota::{build_transfer_tx, GasCoinRef};

let gas = GasCoinRef { object_id, version, digest }; // from RPC
let tx_bytes = build_transfer_tx(&sender, &recipient, amount, &gas, gas_budget, gas_price);

let signature = ledger.sign_tx(&tx_bytes, &path, None)?;
```

Without object data the device will show a blind signing prompt (or reject if blind signing is disabled). For clear signing, pass coin objects so the device can display transfer details:

```rust
use ledger_iota::{ObjectData, Owner};

let objects = vec![ObjectData::gas_coin(
    version,
    contents,
    Owner::AddressOwner(sender),
    previous_transaction,
    storage_rebate,
)];
let signature = ledger.sign_tx(&tx_bytes, &path, Some(&objects))?;
```

## Features

| Feature | Default | Description |
|---------|---------|-------------|
| `hid` | yes | USB HID transport for real Ledger devices |
| `tcp` | no | TCP transport for Speculos simulator |
| `iota-sdk-types` | no | SDK object conversion and SDK return types for `get_pubkey`/`sign_tx` |

```toml
[dependencies]
ledger-iota = "0.1"

# for Speculos testing
ledger-iota = { version = "0.1", features = ["tcp"] }

# with iota-sdk-types integration
ledger-iota = { version = "0.1", features = ["iota-sdk-types"] }
```

### Converting SDK objects for clear signing

With the `iota-sdk-types` feature enabled, you can convert SDK objects directly:

```rust
use ledger_iota::ObjectData;

let sdk_objects: Vec<iota_sdk_types::Object> = /* from RPC */;
let objects: Vec<ObjectData> = sdk_objects
    .into_iter()
    .map(ObjectData::try_from)
    .collect::<Result<_, _>>()?;

let signature = ledger.sign_tx(&tx_bytes, &path, Some(&objects))?;
```

Supported object types: GasCoin, custom coins (`0x2::coin::Coin<T>`), and StakedIota.

## Examples

```sh
cargo run --example version       # print app version
cargo run --example address       # derive address for default path
cargo run --example verify        # verify address on device
cargo run --example pubkeys       # generate range of pubkeys
cargo run --example sign --features tcp  # sign with Speculos
cargo run --example send_iota -- 0x<ADDR> 1000000000  # build & sign IOTA transfer
```

## Testing

Unit tests:

```sh
cargo test --all-features
```

Integration tests require a running Speculos instance:

```sh
speculos --model nanosp /path/to/app-iota.elf
cargo test --features tcp -- --ignored
```

## Supported devices

Nano S, Nano S+, Nano X, Flex, Stax — detected automatically from USB product ID.

Tested with Ledger Nano X on GNU/Linux 6.17.13 (Guix) and IOTA app v1.0.1.

## License

MIT
