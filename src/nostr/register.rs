use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use nostr_sdk::{
    Client, Filter, RelayMessage, RelayPoolNotification, SubscribeAutoCloseOptions, SubscriptionId,
};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Error, Debug)]
pub enum RegisterError {
    #[error("Subscription not found")]
    SubscriptionNotFound,
    #[error(transparent)]
    Client(#[from] nostr_sdk::client::Error),
}

type Result<T> = std::result::Result<T, RegisterError>;

pub type NotificationHandler = Arc<
    dyn Fn(
            RelayPoolNotification,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool>> + Send>>
        + Send
        + Sync,
>;

type HandlerStatus = Arc<RwLock<bool>>;
type HandlerMap = DashMap<SubscriptionId, (NotificationHandler, HandlerStatus)>;

#[derive(Clone)]
pub struct Register {
    handlers: Arc<HandlerMap>,
}

impl Default for Register {
    fn default() -> Self {
        Self::new()
    }
}

impl Register {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(DashMap::new()),
        }
    }

    pub async fn set_stop_flag(&self, sub_id: &SubscriptionId, value: bool) {
        if let Entry::Occupied(mut entry) = self.handlers.entry(sub_id.clone()) {
            let (_, stop_flag) = entry.get_mut();
            let mut stop_flag = stop_flag.write().await;
            *stop_flag = value;
        }
    }
    pub async fn get_sub_flag(&self, sub_id: &SubscriptionId) -> bool {
        if let Some(entry) = self.handlers.get(sub_id) {
            let (_, stop_flag) = entry.value();
            return *stop_flag.read().await;
        }
        false
    }

    pub async fn add_subscription(
        &self,
        client: &Client,
        sub_id: SubscriptionId,
        filters: Vec<Filter>,
        handler: NotificationHandler,
        opts: Option<SubscribeAutoCloseOptions>,
    ) -> Result<()> {
        client
            .subscribe_with_id(sub_id.clone(), filters, opts)
            .await;
        self.handlers
            .insert(sub_id.clone(), (handler, Arc::new(RwLock::new(false))));
        Ok(())
    }

    pub async fn remove_subscription(&self, sub_id: &SubscriptionId) {
        self.handlers.remove(sub_id);
    }

    async fn handle_notification(&self, notification: RelayPoolNotification) -> Result<bool> {
        if let RelayPoolNotification::Message {
            message: RelayMessage::Event {
                subscription_id, ..
            },
            ..
        } = &notification
        {
            if let Some(entry) = self.handlers.get(subscription_id) {
                let (handler, stop_flag) = entry.value();
                let result = (handler)(notification.clone()).await?;
                let stop_flag_val = *stop_flag.read().await;
                return Ok(result || stop_flag_val);
            }
        }
        Ok(false)
    }

    pub async fn handle_notifications(&self, client: &Client) -> Result<()> {
        tracing::info!("Register::handle_notifications");
        client
            .handle_notifications(|notification| {
                let register = self.clone();
                async move {
                    register
                        .handle_notification(notification)
                        .await
                        .map_err(Into::into)
                }
            })
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testhelper::{sleep, test_hander::create_console_log_handler};
    use nostr_sdk::FromBech32;
    use nostr_sdk::{EventId, Filter, PublicKey, SubscriptionId};
    use std::rc::Rc;
    use std::sync::Mutex;
    use wasm_bindgen_futures::spawn_local;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test(async)]
    async fn test_sub_for_two_clients() {
        let _timeout = Some(std::time::Duration::from_secs(5));
        let event_id1 =
            EventId::from_hex("770e3b604de378c67570ce3c521e2fd51c1a59aa85c22ef9aeab7b5f5e2f5e1b")
                .unwrap();
        let event_id2 =
            EventId::from_hex("70cfdf05fa80ce6b4a54668788eef31ff7d5a23b74f54943ec9e5a91cb5806f1")
                .unwrap();
        let client1 = Client::default();
        client1.add_relay("wss://nos.lol").await.unwrap();
        client1.connect().await;
        let client2 = Client::default();
        client2.add_relay("wss://nos.lol").await.unwrap();
        client2.connect().await;
        let console_log_handler: NotificationHandler = create_console_log_handler();

        let filter1 = Filter::new().id(event_id1).limit(1);
        let filter2: Filter = Filter::new().id(event_id2).limit(1);

        let register = Register::default();

        let r = register.clone();
        // Add the subscription for test1
        r.add_subscription(
            &client1,
            SubscriptionId::new("test1"),
            vec![filter1],
            console_log_handler.clone(),
            None,
        )
        .await
        .unwrap();

        // Handle notifications for test1
        let r = register.clone();
        spawn_local(async move {
            r.handle_notifications(&client1).await.unwrap();
        });

        let r = register.clone();
        // Stop handling notifications for test1 after some time
        spawn_local(async move {
            sleep(2000).await.unwrap();
            r.set_stop_flag(&SubscriptionId::new("test1"), true).await;
        });

        let r = register.clone();
        // Add the subscription for test2 after stopping test1
        sleep(3000).await.unwrap();
        r.add_subscription(
            &client2,
            SubscriptionId::new("test2"),
            vec![filter2],
            console_log_handler.clone(),
            None,
        )
        .await
        .unwrap();

        // Handle notifications for test2
        let r = register.clone();
        spawn_local(async move {
            r.handle_notifications(&client2).await.unwrap();
        });

        sleep(5000).await.unwrap();
    }

    #[wasm_bindgen_test(async)]
    async fn test_seen_on_relays() {
        let event_id =
            EventId::from_hex("770e3b604de378c67570ce3c521e2fd51c1a59aa85c22ef9aeab7b5f5e2f5e1b")
                .unwrap();
        let client = Rc::new(Client::default());
        client.add_relay("wss://nos.lol").await.unwrap();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.add_relay("wss://nostr.oxtr.dev").await.unwrap();
        client.connect().await;
        let register = Register::default();
        let filter = Filter::new().id(event_id).limit(1);

        let console_log_handler: NotificationHandler = create_console_log_handler();

        register
            .add_subscription(
                &client,
                SubscriptionId::new("test_seen_on_relays"),
                vec![filter],
                console_log_handler,
                None,
            )
            .await
            .unwrap();

        let cc = Rc::clone(&client);
        spawn_local(async move {
            register.handle_notifications(&cc).await.unwrap();
        });

        sleep(2000).await.unwrap();

        // Query seen on relay
        let relays = client
            .database()
            .event_seen_on_relays(event_id)
            .await
            .unwrap()
            .unwrap();
        console_log!("seen on relays: {:?}", relays);
        assert!(!relays.is_empty());
    }

    #[wasm_bindgen_test(async)]
    async fn test_update_subscription() {
        let brian_search = Filter::new().author(
            PublicKey::from_bech32(
                "npub1tmnfxwvvyx56kt8m904r78umhehwhpgpcpfakelh505r5ve2d2cqa0jccl",
            )
            .unwrap(),
        );
        let filter1 = Filter::new()
            .author(
                PublicKey::from_bech32(
                    "npub1awsnqr5338h497yam5m9hrgh9535yadj9zxglwk55xpsdtsn2c4syjruew",
                )
                .unwrap(),
            )
            .limit(1);
        let filter2 = Filter::new().author(
            PublicKey::from_bech32(
                "npub1vaq95a68j42vwau30ymu56klrkl4g9wxpd7ljsljl8rg3uwd425qyht7a9",
            )
            .unwrap(),
        );
        let client = Client::default();
        client.add_relay("wss://nos.lol").await.unwrap();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.connect().await;
        let register = Register::default();
        let console_log_handler: NotificationHandler = create_console_log_handler();
        register
            .add_subscription(
                &client.clone(),
                SubscriptionId::generate(),
                vec![brian_search],
                console_log_handler.clone(),
                None,
            )
            .await
            .unwrap();

        register
            .add_subscription(
                &client.clone(),
                SubscriptionId::generate(),
                vec![filter2],
                console_log_handler.clone(),
                None,
            )
            .await
            .unwrap();

        // Uncomment the following line to see the logs
        // register.handle_notifications(&client).await.unwrap();
    }

    #[wasm_bindgen_test(async)]
    async fn test_handler_as_closure() {
        let brian_search = Filter::new().author(
            PublicKey::from_bech32(
                "npub1tmnfxwvvyx56kt8m904r78umhehwhpgpcpfakelh505r5ve2d2cqa0jccl",
            )
            .unwrap(),
        );

        let client = Client::default();
        client.add_relay("wss://nos.lol").await.unwrap();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.connect().await;

        // Add a counter with Arc<Mutex<u32>>
        let counter = Arc::new(Mutex::new(0));

        let register = Register::default();
        register
            .add_subscription(
                &client.clone(),
                SubscriptionId::generate(),
                vec![brian_search],
                Arc::new({
                    let counter = Arc::clone(&counter);
                    move |notification| {
                        let counter = Arc::clone(&counter);
                        Box::pin(async move {
                            // console_log!("Received notification: {:?}", notification);
                            let mut counter_lock = counter.lock().unwrap();
                            *counter_lock += 1;
                            console_log!("Counter: {}", *counter_lock);
                            Ok(false)
                        })
                    }
                }),
                None,
            )
            .await
            .unwrap();

        // Uncomment the following line to see the logs
        register.handle_notifications(&client).await.unwrap();
    }
}
