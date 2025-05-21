# Nostr-CRDT

A CRDT (Conflict-free Replicated Data Type) library implemented using Nostr, supporting encrypted data synchronization.

## Features

- Supports three basic CRDT types:
  - LWW-Register (Last-Writer-Wins)
  - G-Counter (Grow-only Counter)
  - G-Set (Grow-only Set)
- Uses Nostr network as the transport layer
- Supports NIP-04 encryption
- Reliable conflict resolution
- Distributed data synchronization without a central server

## Installation

Add the dependency to your Cargo.toml:

```toml
[dependencies]
nostr-crdt = "0.1.0"
```

## Basic Usage

```rust
use nostr_crdt::nostr::crdt::{CrdtManager, CrdtOperation};
use nostr_sdk::{Client, Keys, SecretKey};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create Nostr client
    let secret_key = SecretKey::generate();
    let keys = Keys::new(secret_key);
    let client = Client::new(&keys);
    
    // Add relay
    client.add_relay("wss://relay.damus.io").await?;
    client.connect().await;
    
    // Create CRDT manager
    let signer = client.signer().await?;
    let crdt_manager = CrdtManager::new(Arc::new(client.clone()), signer.clone(), keys.clone());
    
    // Update LWW-Register
    crdt_manager.update_lww_register("username", "capybara").await?;
    
    // Increment counter
    crdt_manager.increment_counter("visitors", 1).await?;
    
    // Add to set
    crdt_manager.add_to_set("tags", "nostr").await?;
    
    Ok(())
}
```

## Performance Tests

This project includes a comprehensive benchmark suite to measure the performance of CRDT operations.

Run the benchmarks:

```bash
cargo bench
```

The benchmarks include:

1. **CRDT Operation Performance**
   - LWW-Register updates and conflict resolution
   - G-Counter increments
   - G-Set additions and idempotence

2. **Serialization/Deserialization Performance**
   - JSON serialization/deserialization of different CRDT types

3. **Encryption/Decryption Performance**
   - NIP-04 encryption/decryption operations

4. **Network Operation Performance**
   - Publishing and processing CRDT operations

## Principles

CRDTs (Conflict-free Replicated Data Types) are special data structures that allow nodes in a distributed system to independently modify data and automatically merge these modifications without conflicts.

In this implementation:

1. Each CRDT operation is serialized to JSON
2. Encrypted using NIP-04 (only the initiator can decrypt)
3. Transmitted via the Nostr network
4. Receivers decrypt and apply the operations

The most important feature is: regardless of the order in which operations arrive, the system will eventually reach a consistent state.

## License

MIT 