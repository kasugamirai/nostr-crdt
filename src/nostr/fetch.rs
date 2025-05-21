use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::{Future, StreamExt};
use gloo_timers::future::TimeoutFuture;
use nostr_indexeddb::database::Order;
use nostr_sdk::{
    Client, Event, EventId, Filter, JsonUtil, Kind, Metadata, NostrSigner, PublicKey, Tag,
    TagStandard, Timestamp,
};
use thiserror::Error;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::Stream;
use wasm_bindgen_futures::spawn_local;

use super::utils::{get_newest_event, get_oldest_event};

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Client(#[from] nostr_sdk::client::Error),
    #[error(transparent)]
    Metadata(#[from] nostr_sdk::types::metadata::Error),
    #[error(transparent)]
    IndexDb(#[from] nostr_indexeddb::IndexedDBError),
    #[error(transparent)]
    Decrypt(#[from] nostr_sdk::nips::nip04::Error),
    #[error(transparent)]
    Signer(#[from] nostr_sdk::signer::Error),
    #[error(transparent)]
    Database(#[from] nostr_indexeddb::database::DatabaseError),
    #[error(transparent)]
    ChannelSend(#[from] tokio::sync::mpsc::error::TrySendError<String>),
    #[error("Event not found")]
    EventNotFound,
}
type Result<T> = std::result::Result<T, Error>;

macro_rules! create_encrypted_filters {
    ($kind:expr, $author:expr, $public_key:expr) => {{
        (
            Filter::new()
                .kind($kind)
                .author($author)
                .pubkey($public_key),
            Filter::new()
                .kind($kind)
                .author($author)
                .pubkey($public_key),
        )
    }};
}

#[derive(Debug)]
pub struct DecryptedMsg {
    /// Id
    pub id: EventId,
    /// Author
    pub pubkey: PublicKey,
    /// Timestamp (seconds):
    pub created_at: Timestamp,
    /// Kind
    pub kind: Kind,
    /// Vector of [`Tag`]
    pub tags: Vec<Tag>,
    /// Content
    pub content: Option<String>,
}

impl From<Event> for DecryptedMsg {
    fn from(event: Event) -> Self {
        Self {
            id: event.id,
            pubkey: event.author(),
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags.clone(),
            content: None,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(clippy::arc_with_non_send_sync)]
pub struct EventPaginator {
    client: Arc<Client>,
    filters: Vec<Filter>,
    oldest_timestamp: Option<Timestamp>,
    done: bool,
    timeout: Option<Duration>,
    page_size: usize,
    last_event_ids: HashSet<EventId>,
    from_db: bool,
}

unsafe impl Send for EventPaginator {}
unsafe impl Sync for EventPaginator {}

impl EventPaginator {
    pub fn new(
        client: Arc<Client>,
        filters: Vec<Filter>,
        timeout: Option<Duration>,
        page_size: usize,
        from_db: bool,
    ) -> Self {
        Self {
            client,
            filters,
            oldest_timestamp: None,
            done: false,
            timeout,
            page_size,
            last_event_ids: HashSet::new(),
            from_db,
        }
    }

    pub fn are_all_event_ids_present(&self, events: &[Event]) -> bool {
        events
            .iter()
            .all(|event| self.last_event_ids.contains(&event.id))
    }

    pub async fn next_page(&mut self) -> Option<Vec<Event>> {
        if self.done {
            return None;
        }

        // Update filters with the oldest timestamp and limit
        let updated_filters: Vec<Filter> = self
            .filters
            .iter()
            .map(|f| {
                let mut f = f.clone();
                if let Some(timestamp) = self.oldest_timestamp {
                    f = f.until(timestamp - 1);
                }
                f = f.limit(self.page_size);
                f
            })
            .collect();

        let events = if self.from_db {
            // Attempt to fetch from the database first
            match self
                .client
                .database()
                .query(updated_filters.clone(), Order::Desc)
                .await
            {
                Ok(events) => events,
                // If database query fails, fall back to fetching from the relay
                Err(err) => {
                    tracing::error!("Database query failed: {:?}", err);
                    self.done = true;
                    return None;
                }
            }
        } else {
            // Directly fetch from the relay
            match self
                .client
                .get_events_of(updated_filters.clone(), self.timeout)
                .await
            {
                Ok(events) => events,
                Err(err) => {
                    tracing::error!("Relay fetch failed: {:?}", err);
                    self.done = true;
                    return None;
                }
            }
        };

        if events.is_empty() || self.are_all_event_ids_present(&events) {
            self.done = true;
            return None;
        }

        // Update the oldest timestamp
        if let Some(oldest_event) = get_oldest_event(&events) {
            self.oldest_timestamp = Some(oldest_event.created_at());
        } else {
            self.done = true;
            return None; // No valid oldest event available
        }

        // Update the filters
        self.filters = updated_filters;
        self.last_event_ids = events.iter().map(|event| event.id).collect();
        Some(events)
    }
}

impl Stream for EventPaginator {
    type Item = Option<Vec<Event>>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let fut = self.next_page();
        futures::pin_mut!(fut);
        match fut.poll(cx) {
            std::task::Poll::Ready(res) => std::task::Poll::Ready(Some(res)),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

pub struct DecryptedMsgPaginator<'a> {
    signer: &'a NostrSigner,
    target_pub_key: PublicKey,
    paginator: EventPaginator,
}

impl<'a> DecryptedMsgPaginator<'a> {
    pub async fn new(
        signer: &'a NostrSigner,
        client: Arc<Client>,
        target_pub_key: PublicKey,
        timeout: Option<Duration>,
        page_size: usize,
        from_db: bool,
    ) -> Result<DecryptedMsgPaginator<'a>> {
        let public_key = signer.public_key().await?;

        let (me, target) =
            create_encrypted_filters!(Kind::EncryptedDirectMessage, target_pub_key, public_key);
        let filters = vec![me, target];

        let paginator = EventPaginator::new(client, filters, timeout, page_size, from_db);
        Ok(DecryptedMsgPaginator {
            signer,
            target_pub_key,
            paginator,
        })
    }

    async fn decrypt_dm_event(&self, event: &Event) -> Result<String> {
        let msg = self
            .signer
            .nip04_decrypt(self.target_pub_key, &event.content)
            .await?;
        Ok(msg)
    }

    async fn convert_events(&self, events: Vec<Event>) -> Result<Vec<DecryptedMsg>> {
        let futures: Vec<_> = events
            .into_iter()
            .map(|event| {
                let self_ref = self;
                async move {
                    let msg = self_ref.decrypt_dm_event(&event).await?;
                    let mut decrypted_msg: DecryptedMsg = event.into();
                    decrypted_msg.content = Some(msg);
                    Ok(decrypted_msg)
                }
            })
            .collect();

        futures::future::try_join_all(futures).await
    }

    pub async fn next_page(&mut self) -> Option<Vec<DecryptedMsg>> {
        if self.paginator.done {
            return None;
        }

        if let Some(events) = self.paginator.next_page().await {
            let decrypt_results = self.convert_events(events).await;
            return decrypt_results.ok();
        }

        None
    }
}

pub async fn get_event_by_id(
    client: &Client,
    event_id: &EventId,
    timeout: Option<std::time::Duration>,
) -> Result<Option<Event>> {
    let filter = Filter::new().id(*event_id).limit(1);
    let events = client.get_events_of(vec![filter], timeout).await?;
    Ok(events.into_iter().next())
}

pub async fn get_events_by_ids(
    client: &Client,
    event_ids: &[EventId],
    timeout: Option<std::time::Duration>,
) -> Result<Vec<Event>> {
    let filters: Vec<Filter> = event_ids.iter().map(|id| Filter::new().id(*id)).collect();
    let events = client.get_events_of(filters, timeout).await?;
    Ok(events)
}

pub async fn get_metadata(
    client: &Client,
    public_key: &PublicKey,
    timeout: Option<Duration>,
) -> Result<Metadata> {
    let filter = Filter::new().author(*public_key).kind(Kind::Metadata);
    let events = client.get_events_of(vec![filter], timeout).await?;

    if let Some(event) = get_newest_event(&events) {
        let metadata = Metadata::from_json(&event.content)?;
        client.database().save_event(event).await?;
        Ok(metadata)
    } else {
        Err(Error::EventNotFound)
    }
}

pub async fn get_zap() {
    todo!()
}

pub async fn get_repost(
    client: &Client,
    event_id: &EventId,
    timeout: Option<std::time::Duration>,
) -> Result<Vec<Event>> {
    let filter = Filter::new().kind(Kind::Repost).event(*event_id);
    let events = client.get_events_of(vec![filter], timeout).await?;
    Ok(events)
}

pub async fn get_reactions(
    client: &Client,
    event_id: &EventId,
    timeout: Option<Duration>,
    is_fetch: bool,
) -> Result<HashMap<String, i32>> {
    let mut reaction_map = HashMap::new();
    let mut events: Vec<Event> = Vec::new();

    let mut reaction_filter = Filter::new().kind(Kind::Reaction).event(*event_id);

    // Get reactions from db
    let db_filter = reaction_filter.clone();
    match client.database().query(vec![db_filter], Order::Desc).await {
        Ok(db_events) => {
            if !db_events.is_empty() {
                events.extend(db_events);
            }
        }
        Err(_) => {
            // is_fetch = true;
        }
    }

    let mut since = None;
    if !events.is_empty() {
        since = Some(events[0].created_at + 1);
    }

    // Get reactions from relay if needed
    if is_fetch {
        if let Some(since) = since {
            reaction_filter = reaction_filter.since(since);
        }

        let relay_events = client.get_events_of(vec![reaction_filter], timeout).await?;
        events.extend(relay_events);
    }

    // Assemble data
    for event in events.iter() {
        let content = event.content().to_string();
        *reaction_map.entry(content).or_insert(0) += 1;
    }

    Ok(reaction_map)
}

pub async fn get_replies(
    client: &Client,
    event_id: &EventId,
    timeout: Option<std::time::Duration>,
) -> Result<Vec<Event>> {
    let filter = Filter::new().kind(Kind::TextNote).event(*event_id);
    let events = client.get_events_of(vec![filter], timeout).await?;
    // TODO: filter out the mentions if necessary
    Ok(events)
}

pub async fn get_following(
    client: &Client,
    public_key: &PublicKey,
    timeout: Option<std::time::Duration>,
) -> Result<Vec<String>> {
    let filter = Filter::new().kind(Kind::ContactList).author(*public_key);
    let events = client.get_events_of(vec![filter], timeout).await?;
    let mut ret: Vec<String> = vec![];
    if let Some(latest_event) = events.iter().max_by_key(|event| event.created_at()) {
        ret.extend(latest_event.tags().iter().filter_map(|tag| {
            if let Some(TagStandard::PublicKey {
                uppercase: false, ..
            }) = <nostr_sdk::Tag as Clone>::clone(tag).to_standardized()
            {
                tag.content().map(String::from)
            } else {
                None
            }
        }));
    }
    Ok(ret)
}

pub async fn get_followers(
    client: Arc<Client>,
    public_key: &PublicKey,
    timeout: Option<std::time::Duration>,
    from_db: bool,
) -> impl Stream<Item = String> {
    let filter = Filter::new().kind(Kind::ContactList).pubkey(*public_key);

    let (tx, rx) = mpsc::unbounded_channel();

    spawn_local({
        let paginator = Arc::new(Mutex::new(EventPaginator::new(
            client,
            vec![filter],
            timeout,
            500,
            from_db,
        )));
        let exit_cond = Arc::new(AtomicBool::new(false));
        async move {
            while !exit_cond.load(Ordering::SeqCst) {
                let mut paginator = paginator.lock().await;
                if let Some(events) = paginator.next_page().await {
                    events.iter().for_each(|event| {
                        let author = event.author().to_hex();
                        if let Err(e) = tx.send(author) {
                            tracing::error!("Failed to send follower: {:?}", e);
                        }
                    });

                    if paginator.done {
                        break;
                    }
                } else {
                    break;
                }
            }

            TimeoutFuture::new(1_00).await;
            exit_cond.store(true, Ordering::SeqCst);
        }
    });

    UnboundedReceiverStream::new(rx).filter_map(|res| async { Some(res) })
}

#[derive(Debug, Clone)]
pub enum NotificationMsg {
    Emoji(Event),
    Reply(Event),
    Repost(Event),
    Quote(Event),
    ZapReceipt(Event),
}

pub struct NotificationPaginator {
    paginator: EventPaginator,
}

impl NotificationPaginator {
    pub fn new(
        client: Arc<Client>,
        public_key: PublicKey,
        timeout: Option<std::time::Duration>,
        page_size: usize,
        from_db: bool,
    ) -> Self {
        let filters = create_notification_filters(&public_key);

        Self {
            paginator: EventPaginator::new(client, filters, timeout, page_size, from_db),
        }
    }

    pub async fn next_page(&mut self) -> Option<Vec<NotificationMsg>> {
        self.paginator
            .next_page()
            .await
            .map(process_notification_events)
    }
}

pub fn create_notification_filters(public_key: &PublicKey) -> Vec<Filter> {
    vec![Filter::new()
        .pubkey(*public_key)
        .kind(Kind::Reaction)
        .kind(Kind::TextNote)
        .kind(Kind::Repost)
        .kind(Kind::ZapReceipt)]
}

pub fn process_notification_events(events: Vec<Event>) -> Vec<NotificationMsg> {
    events
        .into_iter()
        .filter_map(|event| match event.kind() {
            Kind::Reaction => Some(NotificationMsg::Emoji(event)),
            Kind::TextNote => {
                if event.content.contains("nostr:") {
                    Some(NotificationMsg::Quote(event))
                } else {
                    Some(NotificationMsg::Reply(event))
                }
            }
            Kind::Repost => Some(NotificationMsg::Repost(event)),
            Kind::ZapReceipt => Some(NotificationMsg::ZapReceipt(event)),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    pub const NOSTR_DB_NAME: &str = "nostr-db";
    use gloo_timers::future::sleep;
    use nostr_indexeddb::database::Order;
    use nostr_indexeddb::WebDatabase;
    use nostr_sdk::key::SecretKey;
    use nostr_sdk::{Client, ClientBuilder, EventBuilder, FromBech32, Keys};
    use wasm_bindgen_futures::spawn_local;
    use wasm_bindgen_test::*;

    use super::*;
    use crate::nostr::note::{DisplayOrder, ReplyTrees};
    use crate::testhelper::event_from;
    use crate::testhelper::test_data::*;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    async fn test_get_event_by_id() {
        let timeout = Some(std::time::Duration::from_secs(5));
        let event_id =
            EventId::from_hex("ff25d26e734c41fa7ed86d28270628f8fb2f6fb03a23eed3d38502499c1a7a2b")
                .unwrap();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.connect().await;
        let event = get_event_by_id(&client, &event_id, timeout).await.unwrap();
        assert!(event.is_some());
    }

    #[wasm_bindgen_test]
    async fn test_get_replies() {
        let timeout = Some(std::time::Duration::from_secs(5));
        let event_id =
            EventId::from_hex("ff25d26e734c41fa7ed86d28270628f8fb2f6fb03a23eed3d38502499c1a7a2b")
                .unwrap();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.connect().await;
        let replies = get_replies(&client, &event_id, timeout).await.unwrap();
        assert_eq!(replies.len(), 4);
    }

    #[wasm_bindgen_test]
    async fn test_get_replies_into_tree() {
        let timeout = Some(std::time::Duration::from_secs(5));
        let event_id =
            EventId::from_hex("57938b39678af44bc3ae76cf4b815bcdb65ffe71bb84ce35706f0c6fca4ed394")
                .unwrap();
        let client = Client::default();
        client.add_relay("wss://nos.lol").await.unwrap();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.connect().await;
        let root = get_event_by_id(&client, &event_id, timeout)
            .await
            .unwrap()
            .unwrap();
        let replies = get_replies(&client, &event_id, timeout).await.unwrap();
        assert_eq!(replies.len(), 3);
        let mut tree = ReplyTrees::default();
        tree.accept(vec![root]);
        tree.accept(replies);
        let lv1_replies = tree.get_replies(&event_id, Some(DisplayOrder::NewestFirst));
        console_log!("lv1_replies {:?}", lv1_replies);
        assert!(lv1_replies.len() == 3);
    }

    #[wasm_bindgen_test]
    async fn test_get_reactions() {
        let timeout = Some(std::time::Duration::from_secs(5));
        let event_id =
            EventId::from_bech32("note1yht55eufy56v6twj4jzvs4kmplm6k3yayj3yyjzfs9mjhu2vlnms7x3x4h")
                .unwrap();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.connect().await;
        let reactions = get_reactions(&client, &event_id, timeout, true)
            .await
            .unwrap();
        let length = reactions.len();
        console_log!("Reactions: {:?}", reactions);
        assert_eq!(reactions.len(), length);
    }

    // #[wasm_bindgen_test]
    // async fn test_fetch_from_db() {
    //     let db = WebDatabase::open(NOSTR_DB_NAME).await.unwrap();
    //     let client_builder = ClientBuilder::new().database(db);
    //     let client: nostr_sdk::Client = client_builder.build();

    //     //save event to db
    //     let event = event_from(REPLY_WITH_MARKER);
    //     client.database().save_event(&event).await.unwrap();

    //     //query from db
    //     let filter = Filter::new().id(event.id).limit(1);
    //     let event_result = client
    //         .database()
    //         .query(vec![filter], Order::Desc)
    //         .await
    //         .unwrap();
    //     assert!(event_result.len() == 1);
    //     assert!(event_result[0].id == event.id);
    // }

    #[wasm_bindgen_test]
    async fn test_event_page_iterator() {
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.connect().await;

        let public_key = PublicKey::from_bech32(
            "npub1xtscya34g58tk0z605fvr788k263gsu6cy9x0mhnm87echrgufzsevkk5s",
        )
        .unwrap();

        let filter: Filter = Filter::new()
            .kind(Kind::EncryptedDirectMessage)
            .author(public_key);

        let page_size = 10;
        let timeout = Some(std::time::Duration::from_secs(5));
        let mut paginator =
            EventPaginator::new(Arc::new(client), vec![filter], timeout, page_size, false);

        let mut count = 0;
        while let Some(result) = paginator.next_page().await {
            if paginator.done {
                break;
            }
            console_log!("events are: {:?}", result);
            count += result.len();
        }
        assert!(count > 100);
    }

    #[wasm_bindgen_test]
    async fn test_encrypted_direct_message_filters_iterator() {
        let private_key = SecretKey::from_bech32(
            "nsec1qrypzwmxp8r54ctx2x7mhqzh5exca7xd8ssnlfup0js9l6pwku3qacq4u3",
        )
        .unwrap();
        let key = Keys::new(private_key);
        let target_pub_key = PublicKey::from_bech32(
            "npub155pujvquuuy47kpw4j3t49vq4ff9l0uxu97fhph9meuxvwc0r4hq5mdhkf",
        )
        .unwrap();
        let client = Client::new(&key);
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.connect().await;
        let signer = client.signer().await.unwrap();
        let page_size = 3;
        let timeout = Some(std::time::Duration::from_secs(5));
        let mut paginator = DecryptedMsgPaginator::new(
            &signer,
            Arc::new(client),
            target_pub_key,
            timeout,
            page_size,
            false,
        )
        .await
        .unwrap();
        let mut count = 0;
        while let Some(events) = paginator.next_page().await {
            console_log!("events are: {:?}", events);
            for e in &events {
                console_log!("event: {:?}", e.content);
            }
            count += events.len();
        }
        assert!(count > 7);
    }

    #[wasm_bindgen_test]
    async fn test_get_followers() {
        let client = &Client::default();
        let arc_client = Arc::new(client.clone());
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.add_relay("wss://nos.lol").await.unwrap();

        client.connect().await;

        let public_key = PublicKey::from_bech32(
            "npub1zfss807aer0j26mwp2la0ume0jqde3823rmu97ra6sgyyg956e0s6xw445",
        )
        .unwrap();

        let timeout = Some(std::time::Duration::from_secs(5));
        let exit_cond = Arc::new(AtomicBool::new(false));

        let stream = get_followers(arc_client, &public_key, timeout, false).await;

        spawn_local({
            let exit_cond_clone = Arc::clone(&exit_cond);
            async move {
                sleep(Duration::from_secs(1)).await;
                console_log!("Setting exit condition");
                exit_cond_clone.store(true, Ordering::SeqCst);
            }
        });

        let followers = Arc::new(Mutex::new(vec![]));
        let followers_clone = Arc::clone(&followers);
        stream
            .for_each(move |follower| {
                let followers_inner = Arc::clone(&followers_clone);
                async move {
                    followers_inner.lock().await.push(follower);
                    console_log!("followers: {:?}", followers_inner.lock().await.len());
                }
            })
            .await;
        console_log!("exit");

        assert!(!followers.lock().await.is_empty());
    }

    #[wasm_bindgen_test]
    async fn test_get_following() {
        let client = Client::default();
        let client = Arc::new(client);
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.connect().await;

        let public_key = PublicKey::from_bech32(
            "npub1q0uulk2ga9dwkp8hsquzx38hc88uqggdntelgqrtkm29r3ass6fq8y9py9",
        )
        .unwrap();

        let timeout = Some(std::time::Duration::from_secs(5));
        let following = get_following(&client, &public_key, timeout).await.unwrap();
        console_log!("following: {:?}", following);
        assert!(!following.is_empty());
    }

    #[wasm_bindgen_test]
    async fn test_get_notification_paginator() {
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.add_relay("wss://nos.lol").await.unwrap();
        client.connect().await;

        let public_key = PublicKey::from_bech32(
            "npub1zfss807aer0j26mwp2la0ume0jqde3823rmu97ra6sgyyg956e0s6xw445",
        )
        .unwrap();

        let timeout = Some(std::time::Duration::from_secs(5));
        let mut paginator =
            NotificationPaginator::new(Arc::new(client), public_key, timeout, 100, false);
        let mut count = 0;
        loop {
            let result = paginator.next_page().await;
            match result {
                Some(events) => {
                    if paginator.paginator.done {
                        break;
                    }
                    console_log!("events are: {:?}", events);
                    count += events.len();
                }
                None => {
                    console_log!("No more events or an error occurred.");
                    break;
                }
            }
        }
        assert!(count > 0);
    }

    #[wasm_bindgen_test]
    async fn test_get_repost() {
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.connect().await;

        let event_id =
            EventId::from_bech32("note186yr06e9qgd285f9lsj3t56g2nvmqj0ddudgx57sn8k5lqcp5c4q53edv9")
                .unwrap();
        let repost = get_repost(&client, &event_id, None).await.unwrap();
        console_log!("repost: {:?}", repost);
        assert!(!repost.is_empty());
    }
}
