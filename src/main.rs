use nostr_crdt::nostr::crdt::{
    CrdtManager, CrdtOperation, CrdtState, GCounter, GSet, GSetAction, LWWRegister,
};
use nostr_indexeddb::nostr::nips::nip19::ToBech32;
use nostr_sdk::{Client, Keys, SecretKey};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber
    tracing_subscriber::fmt::init();

    info!("CRDT Demo using Nostr");

    // Generate a new private key, or use an existing one
    let secret_key = SecretKey::generate();
    info!("Using private key: {}", secret_key.to_bech32().unwrap());

    let keys = Keys::new(secret_key);

    // Create Nostr client
    let client = Client::new(&keys);

    // Add some relay servers
    client.add_relay("wss://relay.damus.io").await?;
    client.add_relay("wss://nos.lol").await?;
    client.add_relay("wss://nostr.wine").await?;
    client.add_relay("wss://relay.nostr.band").await?;
    client.add_relay("wss://relay.snort.social").await?;

    // Connect to relay servers
    client.connect().await;
    info!("Connected to relay servers");

    // Wait for connection confirmation
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Get signer
    let signer = client.signer().await?;

    // Create CRDT manager
    let crdt_manager = CrdtManager::new(Arc::new(client.clone()), signer.clone(), keys.clone());

    // 1. Demonstrate LWW-Register
    info!("Demonstrating Last-Writer-Wins Register:");

    // Update register
    let event_id = crdt_manager
        .update_lww_register("username", "capybara")
        .await?;
    info!("Published username update event: {}", event_id);

    // Get current value
    let username = crdt_manager.get_register_value("username");
    info!("Current username: {:?}", username);

    // Update to new value
    let event_id = crdt_manager
        .update_lww_register("username", "super_capybara")
        .await?;
    info!("Published new username update event: {}", event_id);

    // Get updated value
    let username = crdt_manager.get_register_value("username");
    info!("Updated username: {:?}", username);

    // 2. Demonstrate G-Counter
    info!("Demonstrating Grow-only Counter:");

    // Increment counter
    let event_id = crdt_manager.increment_counter("visitors", 1).await?;
    info!("Published visitor count increment event: {}", event_id);

    // Increment counter again
    let event_id = crdt_manager.increment_counter("visitors", 5).await?;
    info!("Published visitor count increment event: {}", event_id);

    // Get current count
    let visitors = crdt_manager.get_counter_value("visitors");
    info!("Current visitor count: {:?}", visitors);

    // 3. Demonstrate G-Set
    info!("Demonstrating Grow-only Set:");

    // Add to set
    let event_id = crdt_manager.add_to_set("tags", "nostr").await?;
    info!("Published tag addition event: {}", event_id);

    // Add more to set
    let event_id = crdt_manager.add_to_set("tags", "crdt").await?;
    info!("Published tag addition event: {}", event_id);

    let event_id = crdt_manager.add_to_set("tags", "distributed").await?;
    info!("Published tag addition event: {}", event_id);

    // Get current set
    let tags = crdt_manager.get_set_value("tags");
    info!("Current tag set: {:?}", tags);

    // Wait for a while to ensure events have propagated
    info!("Waiting for event propagation...");
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Create a filter to get CRDT events
    let filter = crdt_manager.get_filter();

    // Fetch events from network
    info!("Fetching CRDT events from network:");

    // Add error handling
    let events = match client
        .get_events_of(vec![filter], Some(Duration::from_secs(10)))
        .await
    {
        Ok(events) => events,
        Err(err) => {
            error!("Error fetching events: {}", err);
            Vec::new() // Return empty array instead of error
        }
    };

    info!("Found {} CRDT events", events.len());
    for (i, event) in events.iter().enumerate() {
        debug!("Event {}: ID: {}", i + 1, event.id);

        // Check if it contains hashtag marker
        let is_crdt_event = event.tags.iter().any(|tag| {
            if let Some(values) = tag.as_vec().get(0..2) {
                return values.len() == 2 && values[0] == "t" && values[1] == "nostr-crdt";
            }
            false
        });

        if !is_crdt_event {
            debug!("Not a CRDT event, skipping");
            continue;
        }

        // Check if content needs decryption
        let content = if event.content.contains("?iv=") {
            debug!("Content is encrypted, attempting to decrypt...");
            // Important: Use sender's public key (event.pubkey) for decryption
            match signer.nip04_decrypt(event.pubkey, &event.content).await {
                Ok(decrypted) => {
                    debug!("Decryption successful");
                    decrypted
                }
                Err(err) => {
                    warn!("Decryption failed: {}", err);
                    event.content.clone()
                }
            }
        } else {
            event.content.clone()
        };

        debug!("Event content: {}", content);
        match serde_json::from_str::<CrdtOperation>(&content) {
            Ok(op) => debug!("Operation: {:?}", op),
            Err(err) => warn!("Could not parse operation: {}", err),
        }
    }

    info!("CRDT demonstration completed");

    // CRDT merge test - Simulate eventual consistency after receiving operations in different orders
    info!("===== CRDT Merge Test =====");

    info!("1. LWW-Register merge test (last writer wins):");

    // Create a simulated CRDT manager, for local testing only
    let mut lww_register = LWWRegister::default();

    // Earlier operation
    let op_a = CrdtOperation::LWWRegister {
        key: "test_key".to_string(),
        value: "Value A".to_string(),
        timestamp: 100,
    };

    // Later operation
    let op_b = CrdtOperation::LWWRegister {
        key: "test_key".to_string(),
        value: "Value B".to_string(),
        timestamp: 200,
    };

    // Simulate Device 1: Apply A then B
    info!("  Device 1: Apply A (timestamp 100) then B (timestamp 200)");
    let mut device1 = LWWRegister::default();
    device1.apply_operation(op_a.clone()).unwrap();
    debug!(
        "    Value after applying A: {:?}",
        device1.get_value("test_key")
    );
    device1.apply_operation(op_b.clone()).unwrap();
    debug!(
        "    Value after applying B: {:?}",
        device1.get_value("test_key")
    );

    // Simulate Device 2: Apply B then A
    info!("  Device 2: Apply B (timestamp 200) then A (timestamp 100)");
    let mut device2 = LWWRegister::default();
    device2.apply_operation(op_b.clone()).unwrap();
    debug!(
        "    Value after applying B: {:?}",
        device2.get_value("test_key")
    );
    device2.apply_operation(op_a.clone()).unwrap();
    debug!(
        "    Value after applying A: {:?}",
        device2.get_value("test_key")
    );

    // Verify final state is consistent
    info!(
        "  Final result: Device 1={:?}, Device 2={:?}",
        device1.get_value("test_key"),
        device2.get_value("test_key")
    );
    info!(
        "  Merge successful: {}",
        device1.get_value("test_key") == device2.get_value("test_key")
    );

    // G-Counter merge test
    info!("2. G-Counter merge test (grow-only counter):");

    // Simulate Device 1: First +3 then +2
    info!("  Device 1: First +3 then +2");
    let mut counter1 = GCounter::default();
    counter1
        .apply_operation(CrdtOperation::GCounter {
            key: "test_counter".to_string(),
            increment: 3,
        })
        .unwrap();
    debug!(
        "    Count after +3: {:?}",
        counter1.get_value("test_counter")
    );

    counter1
        .apply_operation(CrdtOperation::GCounter {
            key: "test_counter".to_string(),
            increment: 2,
        })
        .unwrap();
    debug!(
        "    Count after +2: {:?}",
        counter1.get_value("test_counter")
    );

    // Simulate Device 2: First +2 then +3
    info!("  Device 2: First +2 then +3");
    let mut counter2 = GCounter::default();
    counter2
        .apply_operation(CrdtOperation::GCounter {
            key: "test_counter".to_string(),
            increment: 2,
        })
        .unwrap();
    debug!(
        "    Count after +2: {:?}",
        counter2.get_value("test_counter")
    );

    counter2
        .apply_operation(CrdtOperation::GCounter {
            key: "test_counter".to_string(),
            increment: 3,
        })
        .unwrap();
    debug!(
        "    Count after +3: {:?}",
        counter2.get_value("test_counter")
    );

    // Verify final count is the same
    info!(
        "  Final result: Device 1={:?}, Device 2={:?}",
        counter1.get_value("test_counter"),
        counter2.get_value("test_counter")
    );
    info!(
        "  Merge successful: {}",
        counter1.get_value("test_counter") == counter2.get_value("test_counter")
    );

    // G-Set merge test
    info!("3. G-Set merge test (grow-only set):");

    // Simulate Device 1: Add A, B, C
    info!("  Device 1: Addition order A->B->C");
    let mut set1 = GSet::default();
    set1.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "A".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    debug!("    After adding A: {:?}", set1.get_value("test_set"));

    set1.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "B".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    debug!("    After adding B: {:?}", set1.get_value("test_set"));

    set1.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "C".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    debug!("    After adding C: {:?}", set1.get_value("test_set"));

    // Simulate Device 2: Add C, A, B (different order)
    info!("  Device 2: Addition order C->A->B");
    let mut set2 = GSet::default();
    set2.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "C".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    debug!("    After adding C: {:?}", set2.get_value("test_set"));

    set2.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "A".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    debug!("    After adding A: {:?}", set2.get_value("test_set"));

    set2.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "B".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    debug!("    After adding B: {:?}", set2.get_value("test_set"));

    // Verify final sets are the same
    info!(
        "  Final result: Device 1={:?}, Device 2={:?}",
        set1.get_value("test_set"),
        set2.get_value("test_set")
    );
    info!(
        "  Merge successful: {}",
        set1.get_value("test_set") == set2.get_value("test_set")
    );

    info!("CRDT merge test completed - Demonstrated that regardless of operation order, final state converges to the same result");

    // Disconnect
    client.disconnect().await?;

    Ok(())
}
