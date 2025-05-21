use std::hash::{DefaultHasher, Hash, Hasher};

use indextree::{Arena, NodeId};
use nostr_sdk::{Event, EventId, FromBech32, PublicKey};
use serde::Serialize;

/// Utility function to get all children of a specified node in an Arena.
///
/// # Arguments
/// * `arena` - A reference to an Arena where the nodes are stored.
/// * `node_id` - The NodeId of the parent node whose children are to be collected.
///
/// # Returns
/// A vector of references to the child node data.
pub fn get_children<T>(arena: &Arena<T>, node_id: NodeId) -> Vec<&T> {
    let mut children = Vec::new();
    if let Some(first_child_id) = arena[node_id].first_child() {
        let mut current_id = Some(first_child_id);
        while let Some(node_id) = current_id {
            if let Some(node) = arena.get(node_id) {
                children.push(node.get());
            }
            current_id = arena[node_id].next_sibling();
        }
    }
    children
}

/// Collects all ancestor data of the specified node into a vector.
pub fn get_ancestors<T>(arena: &Arena<T>, node_id: NodeId) -> Vec<&T> {
    let mut ancestors = Vec::new();
    let mut current_id = Some(node_id);

    // Traverse up the tree by continuously going to the parent node
    while let Some(id) = current_id {
        if let Some(parent_id) = arena[id].parent() {
            if let Some(parent) = arena.get(parent_id) {
                ancestors.push(parent.get());
            }
            current_id = Some(parent_id);
        } else {
            break;
        }
    }

    ancestors
}

pub fn hash_filter<T: Serialize>(filter: &T) -> u64 {
    let serialized = serde_json::to_string(filter).unwrap();
    let mut hasher = DefaultHasher::new();
    serialized.hash(&mut hasher);
    hasher.finish()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AddressType {
    Note,
    Mention,
    Nostr, // unknown address type
}

pub fn is_note_address(address: &str) -> AddressType {
    let is_start_nostr = address.starts_with("nostr:");
    if is_start_nostr {
        let id = address.strip_prefix("nostr:").unwrap();
        let is_note = id.starts_with("note") && EventId::from_bech32(id).is_ok();
        let is_mention = id.starts_with("npub") && PublicKey::from_bech32(id).is_ok();
        if is_note {
            return AddressType::Note;
        } else if is_mention {
            return AddressType::Mention;
        }
    }
    AddressType::Nostr
}

pub fn get_newest_event(events: &[Event]) -> Option<&Event> {
    events.iter().max_by_key(|event| event.created_at())
}

pub fn get_oldest_event(events: &[Event]) -> Option<&Event> {
    events.iter().min_by_key(|event| event.created_at())
}
/*
pub async fn query_events_from_db(
    client: &Client,
    filters: Vec<Filter>,
) -> Result<Vec<Event>, Error> {
    let events = client.database().query(filters, Order::Desc).await?;
    Ok(events)
}
*/
