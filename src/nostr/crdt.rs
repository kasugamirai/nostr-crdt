use nostr_sdk::{Event, EventBuilder, EventId, Keys, Kind, NostrSigner, Tag, TagKind, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Client(#[from] nostr_sdk::client::Error),
    #[error(transparent)]
    Signer(#[from] nostr_sdk::signer::Error),
    #[error(transparent)]
    Publish(#[from] super::publish::Error),
    #[error("Invalid CRDT operation")]
    InvalidOperation,
    #[error("Serialization error")]
    SerializationError,
    #[error("Keys not available")]
    KeysNotAvailable,
    #[error(transparent)]
    EventBuilder(#[from] nostr_sdk::event::builder::Error),
}

type Result<T> = std::result::Result<T, Error>;

// CRDT operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrdtOperation {
    // Last-Writer-Wins register operation
    LWWRegister {
        key: String,
        value: String,
        timestamp: u64,
    },
    // Grow-only counter operation
    GCounter {
        key: String,
        increment: u64,
    },
    // Add-only set operation
    GSet {
        key: String,
        value: String,
        action: GSetAction,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GSetAction {
    Add,
}

// CRDT state interface
pub trait CrdtState: Send + Sync {
    fn apply_operation(&mut self, op: CrdtOperation) -> Result<()>;
    fn get_value(&self, key: &str) -> Option<String>;
}

// Last-Writer-Wins Register implementation
#[derive(Debug, Clone, Default)]
pub struct LWWRegister {
    registers: HashMap<String, (String, u64)>, // key -> (value, timestamp)
}

impl CrdtState for LWWRegister {
    fn apply_operation(&mut self, op: CrdtOperation) -> Result<()> {
        match op {
            CrdtOperation::LWWRegister {
                key,
                value,
                timestamp,
            } => {
                match self.registers.get(&key) {
                    Some((_, existing_ts)) if *existing_ts >= timestamp => {
                        // Ignore older or same timestamp updates
                        Ok(())
                    }
                    _ => {
                        // Apply newer update
                        self.registers.insert(key, (value, timestamp));
                        Ok(())
                    }
                }
            }
            _ => Err(Error::InvalidOperation),
        }
    }

    fn get_value(&self, key: &str) -> Option<String> {
        self.registers.get(key).map(|(value, _)| value.clone())
    }
}

// Grow-only Counter implementation
#[derive(Debug, Clone, Default)]
pub struct GCounter {
    counters: HashMap<String, u64>, // key -> count
}

impl CrdtState for GCounter {
    fn apply_operation(&mut self, op: CrdtOperation) -> Result<()> {
        match op {
            CrdtOperation::GCounter { key, increment } => {
                let count = self.counters.entry(key).or_insert(0);
                *count += increment;
                Ok(())
            }
            _ => Err(Error::InvalidOperation),
        }
    }

    fn get_value(&self, key: &str) -> Option<String> {
        self.counters.get(key).map(|count| count.to_string())
    }
}

// Grow-only Set implementation
#[derive(Debug, Clone, Default)]
pub struct GSet {
    sets: HashMap<String, Vec<String>>, // key -> set of values
}

impl CrdtState for GSet {
    fn apply_operation(&mut self, op: CrdtOperation) -> Result<()> {
        match op {
            CrdtOperation::GSet {
                key,
                value,
                action: GSetAction::Add,
            } => {
                let set = self.sets.entry(key).or_default();
                if !set.contains(&value) {
                    set.push(value);
                }
                Ok(())
            }
            _ => Err(Error::InvalidOperation),
        }
    }

    fn get_value(&self, key: &str) -> Option<String> {
        self.sets
            .get(key)
            .map(|set| serde_json::to_string(set).unwrap_or_default())
    }
}

// Main CRDT manager
pub struct CrdtManager {
    client: Arc<nostr_sdk::Client>,
    signer: NostrSigner,
    keys: Keys,
    lww_registers: Arc<Mutex<LWWRegister>>,
    g_counters: Arc<Mutex<GCounter>>,
    g_sets: Arc<Mutex<GSet>>,
    crdt_kind: Kind,
}

impl CrdtManager {
    pub fn new(client: Arc<nostr_sdk::Client>, signer: NostrSigner, keys: Keys) -> Self {
        Self {
            client,
            signer,
            keys,
            lww_registers: Arc::new(Mutex::new(LWWRegister::default())),
            g_counters: Arc::new(Mutex::new(GCounter::default())),
            g_sets: Arc::new(Mutex::new(GSet::default())),
            crdt_kind: Kind::TextNote, // Use standard TextNote Kind instead of custom Kind
        }
    }

    // Process incoming Nostr events containing CRDT operations
    pub async fn process_event(&self, event: &Event) -> Result<()> {
        if event.kind != self.crdt_kind {
            return Ok(());
        }

        let content = if event.content.contains("?iv=") {
            // Content that needs decryption
            match self
                .signer
                .nip04_decrypt(event.pubkey, &event.content)
                .await
            {
                Ok(decrypted) => decrypted,
                Err(_) => return Err(Error::SerializationError),
            }
        } else {
            event.content.clone()
        };

        let op: CrdtOperation =
            serde_json::from_str(&content).map_err(|_| Error::SerializationError)?;

        match &op {
            CrdtOperation::LWWRegister { .. } => {
                self.lww_registers.lock().unwrap().apply_operation(op)
            }
            CrdtOperation::GCounter { .. } => self.g_counters.lock().unwrap().apply_operation(op),
            CrdtOperation::GSet { .. } => self.g_sets.lock().unwrap().apply_operation(op),
        }
    }

    // Publish CRDT operation with encryption
    async fn publish_encrypted_crdt_operation(
        &self,
        op: &CrdtOperation,
        tags: Vec<Tag>,
    ) -> Result<EventId> {
        // Serialize operation
        let content = serde_json::to_string(&op).map_err(|_| Error::SerializationError)?;

        // Get own public key and encrypt content
        let my_pubkey = self.signer.public_key().await?;
        let encrypted_content = self.signer.nip04_encrypt(my_pubkey, &content).await?;

        // Create event - add CRDT specific tags
        let mut all_tags = tags;
        // Add hashtag for CRDT operation identification
        all_tags.push(Tag::hashtag("nostr-crdt"));

        let event =
            EventBuilder::new(self.crdt_kind, &encrypted_content, all_tags).to_event(&self.keys)?;

        // Send event with retry logic
        let mut retry_count = 0;
        let max_retries = 3;
        let mut last_error = None;

        while retry_count < max_retries {
            match self.client.send_event(event.clone()).await {
                Ok(_) => {
                    return Ok(event.id);
                }
                Err(err) => {
                    last_error = Some(err);
                    retry_count += 1;
                    if retry_count < max_retries {
                        // Wait before retrying
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                }
            }
        }

        // All retries failed
        Err(Error::Client(last_error.unwrap()))
    }

    // Create and publish a LWW-Register update
    pub async fn update_lww_register(&self, key: &str, value: &str) -> Result<EventId> {
        let now = Timestamp::now().as_u64();
        let op = CrdtOperation::LWWRegister {
            key: key.to_string(),
            value: value.to_string(),
            timestamp: now,
        };

        // Apply operation locally first
        self.lww_registers
            .lock()
            .unwrap()
            .apply_operation(op.clone())?;

        // Then publish to network
        let tags = vec![Tag::custom(TagKind::from("c"), ["crdt", "lww"])];
        self.publish_encrypted_crdt_operation(&op, tags).await
    }

    // Create and publish a G-Counter increment
    pub async fn increment_counter(&self, key: &str, increment: u64) -> Result<EventId> {
        let op = CrdtOperation::GCounter {
            key: key.to_string(),
            increment,
        };

        // Apply operation locally first
        self.g_counters
            .lock()
            .unwrap()
            .apply_operation(op.clone())?;

        // Then publish to network
        let tags = vec![Tag::custom(TagKind::from("c"), ["crdt", "gcounter"])];
        self.publish_encrypted_crdt_operation(&op, tags).await
    }

    // Create and publish a G-Set add operation
    pub async fn add_to_set(&self, key: &str, value: &str) -> Result<EventId> {
        let op = CrdtOperation::GSet {
            key: key.to_string(),
            value: value.to_string(),
            action: GSetAction::Add,
        };

        // Apply operation locally first
        self.g_sets.lock().unwrap().apply_operation(op.clone())?;

        // Then publish to network
        let tags = vec![Tag::custom(TagKind::from("c"), ["crdt", "gset"])];
        self.publish_encrypted_crdt_operation(&op, tags).await
    }

    // Get value from LWW-Register
    pub fn get_register_value(&self, key: &str) -> Option<String> {
        self.lww_registers.lock().unwrap().get_value(key)
    }

    // Get value from G-Counter
    pub fn get_counter_value(&self, key: &str) -> Option<String> {
        self.g_counters.lock().unwrap().get_value(key)
    }

    // Get value from G-Set
    pub fn get_set_value(&self, key: &str) -> Option<String> {
        self.g_sets.lock().unwrap().get_value(key)
    }

    // Create a filter to subscribe to CRDT events
    pub fn get_filter(&self) -> nostr_sdk::Filter {
        // Update filter to include application-specific tags
        nostr_sdk::Filter::new()
            .kind(self.crdt_kind)
            .hashtag("nostr-crdt") // Use hashtag as alternative
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lww_register() {
        let mut lww = LWWRegister::default();

        // Apply operations in timestamp order
        lww.apply_operation(CrdtOperation::LWWRegister {
            key: "test".to_string(),
            value: "value1".to_string(),
            timestamp: 100,
        })
        .unwrap();

        lww.apply_operation(CrdtOperation::LWWRegister {
            key: "test".to_string(),
            value: "value2".to_string(),
            timestamp: 200,
        })
        .unwrap();

        // This should be ignored (older timestamp)
        lww.apply_operation(CrdtOperation::LWWRegister {
            key: "test".to_string(),
            value: "value3".to_string(),
            timestamp: 150,
        })
        .unwrap();

        assert_eq!(lww.get_value("test"), Some("value2".to_string()));
    }

    #[test]
    fn test_g_counter() {
        let mut counter = GCounter::default();

        counter
            .apply_operation(CrdtOperation::GCounter {
                key: "visitors".to_string(),
                increment: 1,
            })
            .unwrap();

        counter
            .apply_operation(CrdtOperation::GCounter {
                key: "visitors".to_string(),
                increment: 1,
            })
            .unwrap();

        counter
            .apply_operation(CrdtOperation::GCounter {
                key: "downloads".to_string(),
                increment: 5,
            })
            .unwrap();

        assert_eq!(counter.get_value("visitors"), Some("2".to_string()));
        assert_eq!(counter.get_value("downloads"), Some("5".to_string()));
    }

    #[test]
    fn test_g_set() {
        let mut set = GSet::default();

        set.apply_operation(CrdtOperation::GSet {
            key: "users".to_string(),
            value: "alice".to_string(),
            action: GSetAction::Add,
        })
        .unwrap();

        set.apply_operation(CrdtOperation::GSet {
            key: "users".to_string(),
            value: "bob".to_string(),
            action: GSetAction::Add,
        })
        .unwrap();

        // Duplicate add (should be idempotent)
        set.apply_operation(CrdtOperation::GSet {
            key: "users".to_string(),
            value: "alice".to_string(),
            action: GSetAction::Add,
        })
        .unwrap();

        let value = set.get_value("users").unwrap();
        let parsed: Vec<String> = serde_json::from_str(&value).unwrap();

        assert_eq!(parsed.len(), 2);
        assert!(parsed.contains(&"alice".to_string()));
        assert!(parsed.contains(&"bob".to_string()));
    }
}
