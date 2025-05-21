use nostr_crdt::nostr::crdt::{
    CrdtManager, CrdtOperation, CrdtState, GCounter, GSet, GSetAction, LWWRegister,
};
use nostr_indexeddb::nostr::nips::nip19::ToBech32;
use nostr_sdk::{Client, Keys, SecretKey};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("CRDT Demo using Nostr");

    // Generate a new private key, or use an existing one
    let secret_key = SecretKey::generate();
    println!("Using private key: {}", secret_key.to_bech32().unwrap());

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
    println!("Connected to relay servers");

    // Wait for connection confirmation
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Get signer
    let signer = client.signer().await?;

    // Create CRDT manager
    let crdt_manager = CrdtManager::new(Arc::new(client.clone()), signer.clone(), keys.clone());

    // 1. Demonstrate LWW-Register
    println!("\nDemonstrating Last-Writer-Wins Register:");

    // Update register
    let event_id = crdt_manager
        .update_lww_register("username", "capybara")
        .await?;
    println!("Published username update event: {}", event_id);

    // Get current value
    let username = crdt_manager.get_register_value("username");
    println!("Current username: {:?}", username);

    // Update to new value
    let event_id = crdt_manager
        .update_lww_register("username", "super_capybara")
        .await?;
    println!("Published new username update event: {}", event_id);

    // Get updated value
    let username = crdt_manager.get_register_value("username");
    println!("Updated username: {:?}", username);

    // 2. Demonstrate G-Counter
    println!("\nDemonstrating Grow-only Counter:");

    // Increment counter
    let event_id = crdt_manager.increment_counter("visitors", 1).await?;
    println!("Published visitor count increment event: {}", event_id);

    // Increment counter again
    let event_id = crdt_manager.increment_counter("visitors", 5).await?;
    println!("Published visitor count increment event: {}", event_id);

    // Get current count
    let visitors = crdt_manager.get_counter_value("visitors");
    println!("Current visitor count: {:?}", visitors);

    // 3. Demonstrate G-Set
    println!("\nDemonstrating Grow-only Set:");

    // Add to set
    let event_id = crdt_manager.add_to_set("tags", "nostr").await?;
    println!("Published tag addition event: {}", event_id);

    // Add more to set
    let event_id = crdt_manager.add_to_set("tags", "crdt").await?;
    println!("Published tag addition event: {}", event_id);

    let event_id = crdt_manager.add_to_set("tags", "distributed").await?;
    println!("Published tag addition event: {}", event_id);

    // Get current set
    let tags = crdt_manager.get_set_value("tags");
    println!("Current tag set: {:?}", tags);

    // Wait for a while to ensure events have propagated
    println!("\nWaiting for event propagation...");
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Create a filter to get CRDT events
    let filter = crdt_manager.get_filter();

    // Fetch events from network
    println!("\nFetching CRDT events from network:");

    // Add error handling
    let events = match client
        .get_events_of(vec![filter], Some(Duration::from_secs(10)))
        .await
    {
        Ok(events) => events,
        Err(err) => {
            println!("Error fetching events: {}", err);
            Vec::new() // Return empty array instead of error
        }
    };

    println!("Found {} CRDT events", events.len());
    for (i, event) in events.iter().enumerate() {
        println!("Event {}: ID: {}", i + 1, event.id);

        // Check if it contains hashtag marker
        let is_crdt_event = event.tags.iter().any(|tag| {
            if let Some(values) = tag.as_vec().get(0..2) {
                return values.len() == 2 && values[0] == "t" && values[1] == "nostr-crdt";
            }
            false
        });

        if !is_crdt_event {
            println!("Not a CRDT event, skipping");
            continue;
        }

        // Check if content needs decryption
        let content = if event.content.contains("?iv=") {
            println!("Content is encrypted, attempting to decrypt...");
            // Important: Use sender's public key (event.pubkey) for decryption
            match signer.nip04_decrypt(event.pubkey, &event.content).await {
                Ok(decrypted) => {
                    println!("Decryption successful");
                    decrypted
                }
                Err(err) => {
                    println!("Decryption failed: {}", err);
                    event.content.clone()
                }
            }
        } else {
            event.content.clone()
        };

        println!("Event content: {}", content);
        match serde_json::from_str::<CrdtOperation>(&content) {
            Ok(op) => println!("Operation: {:?}", op),
            Err(err) => println!("Could not parse operation: {}", err),
        }
    }

    println!("\nCRDT demonstration completed");

    // CRDT merge test - Simulate eventual consistency after receiving operations in different orders
    println!("\n===== CRDT Merge Test =====");

    println!("1. LWW-Register merge test (last writer wins):");

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
    println!("  Device 1: Apply A (timestamp 100) then B (timestamp 200)");
    let mut device1 = LWWRegister::default();
    device1.apply_operation(op_a.clone()).unwrap();
    println!(
        "    Value after applying A: {:?}",
        device1.get_value("test_key")
    );
    device1.apply_operation(op_b.clone()).unwrap();
    println!(
        "    Value after applying B: {:?}",
        device1.get_value("test_key")
    );

    // Simulate Device 2: Apply B then A
    println!("  Device 2: Apply B (timestamp 200) then A (timestamp 100)");
    let mut device2 = LWWRegister::default();
    device2.apply_operation(op_b.clone()).unwrap();
    println!(
        "    Value after applying B: {:?}",
        device2.get_value("test_key")
    );
    device2.apply_operation(op_a.clone()).unwrap();
    println!(
        "    Value after applying A: {:?}",
        device2.get_value("test_key")
    );

    // Verify final state is consistent
    println!(
        "  Final result: Device 1={:?}, Device 2={:?}",
        device1.get_value("test_key"),
        device2.get_value("test_key")
    );
    println!(
        "  Merge successful: {}",
        device1.get_value("test_key") == device2.get_value("test_key")
    );

    // G-Counter merge test
    println!("\n2. G-Counter merge test (grow-only counter):");

    // Simulate Device 1: First +3 then +2
    println!("  Device 1: First +3 then +2");
    let mut counter1 = GCounter::default();
    counter1
        .apply_operation(CrdtOperation::GCounter {
            key: "test_counter".to_string(),
            increment: 3,
        })
        .unwrap();
    println!(
        "    Count after +3: {:?}",
        counter1.get_value("test_counter")
    );

    counter1
        .apply_operation(CrdtOperation::GCounter {
            key: "test_counter".to_string(),
            increment: 2,
        })
        .unwrap();
    println!(
        "    Count after +2: {:?}",
        counter1.get_value("test_counter")
    );

    // Simulate Device 2: First +2 then +3
    println!("  Device 2: First +2 then +3");
    let mut counter2 = GCounter::default();
    counter2
        .apply_operation(CrdtOperation::GCounter {
            key: "test_counter".to_string(),
            increment: 2,
        })
        .unwrap();
    println!(
        "    Count after +2: {:?}",
        counter2.get_value("test_counter")
    );

    counter2
        .apply_operation(CrdtOperation::GCounter {
            key: "test_counter".to_string(),
            increment: 3,
        })
        .unwrap();
    println!(
        "    Count after +3: {:?}",
        counter2.get_value("test_counter")
    );

    // Verify final count is the same
    println!(
        "  Final result: Device 1={:?}, Device 2={:?}",
        counter1.get_value("test_counter"),
        counter2.get_value("test_counter")
    );
    println!(
        "  Merge successful: {}",
        counter1.get_value("test_counter") == counter2.get_value("test_counter")
    );

    // G-Set merge test
    println!("\n3. G-Set merge test (grow-only set):");

    // Simulate Device 1: Add A, B, C
    println!("  Device 1: Addition order A->B->C");
    let mut set1 = GSet::default();
    set1.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "A".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    println!("    After adding A: {:?}", set1.get_value("test_set"));

    set1.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "B".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    println!("    After adding B: {:?}", set1.get_value("test_set"));

    set1.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "C".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    println!("    After adding C: {:?}", set1.get_value("test_set"));

    // Simulate Device 2: Add C, A, B (different order)
    println!("  Device 2: Addition order C->A->B");
    let mut set2 = GSet::default();
    set2.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "C".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    println!("    After adding C: {:?}", set2.get_value("test_set"));

    set2.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "A".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    println!("    After adding A: {:?}", set2.get_value("test_set"));

    set2.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "B".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    println!("    After adding B: {:?}", set2.get_value("test_set"));

    // Verify final sets are the same
    println!(
        "  Final result: Device 1={:?}, Device 2={:?}",
        set1.get_value("test_set"),
        set2.get_value("test_set")
    );
    println!(
        "  Merge successful: {}",
        set1.get_value("test_set") == set2.get_value("test_set")
    );

    println!("\nCRDT merge test completed - Demonstrated that regardless of operation order, final state converges to the same result");

    // Disconnect
    client.disconnect().await?;

    Ok(())
}
