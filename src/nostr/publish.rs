use nostr_sdk::nips::nip65::RelayMetadata;
use nostr_sdk::nips::nip94::FileMetadata;
use nostr_sdk::{
    Client, Contact, Event, EventBuilder, EventId, Filter, Kind, Metadata, NostrSigner, PublicKey,
    Tag, TagStandard, Timestamp, UncheckedUrl, Url,
};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Client(#[from] nostr_sdk::client::Error),
    #[error(transparent)]
    Signer(#[from] nostr_sdk::signer::Error),
}

type Result<T> = std::result::Result<T, Error>;

macro_rules! sign_and_send_event {
    ($client:expr, $signer:expr, $builder:expr) => {{
        let event = $signer.sign_event_builder($builder).await?;
        let eid = $client.send_event(event).await?;
        Ok(eid)
    }};
}

pub async fn publish_text_note(
    client: &Client,
    signer: &NostrSigner,
    content: &str,
    tags: Vec<Tag>,
) -> Result<EventId> {
    let builder = EventBuilder::text_note(content, tags).custom_created_at(Timestamp::now());
    sign_and_send_event!(client, signer, builder)
}

pub async fn repost(
    client: &Client,
    signer: &NostrSigner,
    event: &Event,
    url: Option<UncheckedUrl>,
) -> Result<EventId> {
    let builder = EventBuilder::repost(event, url);
    sign_and_send_event!(client, signer, builder)
}

pub async fn reaction(
    client: &Client,
    signer: &NostrSigner,
    event: &Event,
    reaction: &str,
) -> Result<EventId> {
    let builder = EventBuilder::reaction(event, reaction);
    sign_and_send_event!(client, signer, builder)
}

pub async fn new_channel(
    client: &Client,
    signer: &NostrSigner,
    metadata: &Metadata,
) -> Result<EventId> {
    let builder = EventBuilder::channel(metadata);
    sign_and_send_event!(client, signer, builder)
}

pub async fn set_channel_metadata(
    client: &Client,
    signer: &NostrSigner,
    channel_id: EventId,
    metadata: &Metadata,
    url: Option<Url>,
) -> Result<EventId> {
    let builder = EventBuilder::channel_metadata(channel_id, url, metadata);
    sign_and_send_event!(client, signer, builder)
}

pub async fn send_channel_msg(
    client: &Client,
    signer: &NostrSigner,
    channel_id: EventId,
    msg: &str,
    relay_url: Url,
) -> Result<EventId> {
    let builder = EventBuilder::channel_msg(channel_id, relay_url, msg);
    sign_and_send_event!(client, signer, builder)
}

pub async fn file_metadata(
    client: &Client,
    signer: &NostrSigner,
    metadata: FileMetadata,
    description: &str,
) -> Result<EventId> {
    let builder = EventBuilder::file_metadata(description, metadata);
    sign_and_send_event!(client, signer, builder)
}

pub async fn send_private_msg(
    client: &Client,
    signer: &NostrSigner,
    receiver: PublicKey,
    message: &str,
    reply_to: Option<EventId>,
) -> Result<EventId> {
    let builder = EventBuilder::private_msg_rumor(receiver, message, reply_to);
    sign_and_send_event!(client, signer, builder)
}

pub async fn delete_event(
    client: &Client,
    signer: &NostrSigner,
    event_ids: Vec<EventId>,
) -> Result<EventId> {
    let builder = EventBuilder::delete(event_ids);
    sign_and_send_event!(client, signer, builder)
}

pub async fn set_relay_list(
    client: &Client,
    signer: &NostrSigner,
    relays: Vec<(Url, Option<RelayMetadata>)>,
) -> Result<EventId> {
    let builder = EventBuilder::relay_list(relays);
    sign_and_send_event!(client, signer, builder)
}

pub async fn set_contact_list(
    client: &Client,
    signer: &NostrSigner,
    contacts: Vec<Contact>,
) -> Result<EventId> {
    let builder = EventBuilder::contact_list(contacts);
    sign_and_send_event!(client, signer, builder)
}

pub async fn unfollow(
    client: &Client,
    signer: &NostrSigner,
    followee: PublicKey,
    timeout: Option<Duration>,
) -> Result<EventId> {
    let contacts = get_contact_list(client, signer, timeout).await?;
    let contacts: Vec<Contact> = contacts
        .into_iter()
        .filter(|contact| contact.public_key != followee)
        .collect();
    let builder = EventBuilder::contact_list(contacts);
    sign_and_send_event!(client, signer, builder)
}

pub async fn follow(
    client: &Client,
    signer: &NostrSigner,
    followee: PublicKey,
    timeout: Option<Duration>,
    relay_url: Option<UncheckedUrl>,
    alias: Option<String>,
) -> Result<EventId> {
    let contacts = get_contact_list(client, signer, timeout).await?;
    let contacts: Vec<Contact> = contacts
        .into_iter()
        .filter(|contact| contact.public_key != followee)
        .collect();
    let contact = Contact::new(followee, relay_url, alias);
    let mut contacts = contacts;
    contacts.push(contact);
    let builder = EventBuilder::contact_list(contacts);
    sign_and_send_event!(client, signer, builder)
}

async fn get_contact_list_filters(signer: &NostrSigner) -> Result<Vec<Filter>> {
    let public_key = signer.public_key().await?;
    let filter: Filter = Filter::new()
        .author(public_key)
        .kind(Kind::ContactList)
        .limit(1);
    Ok(vec![filter])
}

async fn get_contact_list(
    client: &Client,
    signer: &NostrSigner,
    timeout: Option<Duration>,
) -> Result<Vec<Contact>> {
    let mut contact_list: Vec<Contact> = Vec::new();
    let filters = get_contact_list_filters(signer).await?;
    let events: Vec<Event> = client.get_events_of(filters, timeout).await?;

    for event in events.into_iter() {
        for tag in event.into_iter_tags() {
            if let Some(TagStandard::PublicKey {
                public_key,
                relay_url,
                alias,
                uppercase: false,
            }) = tag.to_standardized()
            {
                contact_list.push(Contact::new(public_key, relay_url, alias))
            }
        }
    }

    Ok(contact_list)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use nostr_sdk::bitcoin::hashes::sha256::Hash as Sha256Hash;
    use nostr_sdk::{EventId, Filter, FromBech32, Keys, SecretKey, ToBech32};
    use wasm_bindgen_test::*;

    use super::*;
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_publish_text_note() {
        let private_key = SecretKey::from_bech32(
            "nsec19y0u0kgzwx4ygxpk04ktl6uc2daq2mts9w0rk2qrxnru5hhvpjeq20awgp",
        )
        .unwrap();
        let key = Keys::new(private_key);
        let signer = &key.into();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.connect().await;
        let result = publish_text_note(&client, signer, "Hello, world!", vec![]).await;
        assert!(result.is_ok());
    }

    #[wasm_bindgen_test]
    async fn test_repost() {
        let private_key = SecretKey::from_bech32(
            "nsec1qrypzwmxp8r54ctx2x7mhqzh5exca7xd8ssnlfup0js9l6pwku3qacq4u3",
        )
        .unwrap();
        let key = Keys::new(private_key);
        let signer = &key.into();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        let eid =
            EventId::from_bech32("note1zlsz37aggmsc2nfzqjdsdw77qwyfqm3erxag5f75nz8tndkvs0uqllhywm")
                .unwrap();
        let f = Filter::new().event(eid);
        client.connect().await;
        let event = client.get_events_of(vec![f], None).await.unwrap();
        //let url = UncheckedUrl::from("wss://relay.damus.io");
        let result = repost(&client, signer, &event[0], None).await;
        assert!(result.is_ok());
    }

    #[wasm_bindgen_test]
    async fn test_reaction() {
        let private_key = SecretKey::from_bech32(
            "nsec1qrypzwmxp8r54ctx2x7mhqzh5exca7xd8ssnlfup0js9l6pwku3qacq4u3",
        )
        .unwrap();
        let key = Keys::new(private_key);
        let signer = &key.into();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        let eid =
            EventId::from_bech32("note1zlsz37aggmsc2nfzqjdsdw77qwyfqm3erxag5f75nz8tndkvs0uqllhywm")
                .unwrap();
        let f = Filter::new().event(eid);
        client.connect().await;
        let event = client.get_events_of(vec![f], None).await.unwrap();
        let result = reaction(&client, signer, &event[0], "👍").await;
        if let Ok(event_id) = &result {
            console_log!("Event ID: {:?}", event_id.to_bech32());
        } else if let Err(e) = &result {
            console_log!("Error: {:?}", e);
        }
        assert!(result.is_ok());
    }

    #[wasm_bindgen_test]
    async fn test_new_channel() {
        let private_key = SecretKey::from_bech32(
            "nsec1qrypzwmxp8r54ctx2x7mhqzh5exca7xd8ssnlfup0js9l6pwku3qacq4u3",
        )
        .unwrap();
        let key = Keys::new(private_key);
        let signer = &key.into();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        let metadata = Metadata::new();
        client.connect().await;
        let result = new_channel(&client, signer, &metadata).await;
        assert!(result.is_ok());
    }

    #[wasm_bindgen_test]
    async fn test_set_channel_metadata() {
        let private_key = SecretKey::from_bech32(
            "nsec1qrypzwmxp8r54ctx2x7mhqzh5exca7xd8ssnlfup0js9l6pwku3qacq4u3",
        )
        .unwrap();
        let key = Keys::new(private_key);
        let signer = &key.into();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        let metadata = Metadata::new();
        let channel_id =
            EventId::from_bech32("note1zlsz37aggmsc2nfzqjdsdw77qwyfqm3erxag5f75nz8tndkvs0uqllhywm")
                .unwrap();
        client.connect().await;
        let result = set_channel_metadata(&client, signer, channel_id, &metadata, None).await;
        assert!(result.is_ok());
    }

    #[wasm_bindgen_test]
    async fn test_send_channel_msg() {
        let private_key = SecretKey::from_bech32(
            "nsec1qrypzwmxp8r54ctx2x7mhqzh5exca7xd8ssnlfup0js9l6pwku3qacq4u3",
        )
        .unwrap();
        let key = Keys::new(private_key);
        let signer = &key.into();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        let channel_id =
            EventId::from_bech32("note1zlsz37aggmsc2nfzqjdsdw77qwyfqm3erxag5f75nz8tndkvs0uqllhywm")
                .unwrap();
        let url = Url::parse("wss://relay.damus.io").unwrap();
        client.connect().await;
        let result = send_channel_msg(&client, signer, channel_id, "Hello, world!", url).await;
        assert!(result.is_ok());
    }

    #[wasm_bindgen_test]
    async fn test_file_metadata() {
        const IMAGE_URL: &str = "https://image.nostr.build/99a95fcb4b7a2591ad32467032c52a62d90a204d3b176bc2459ad7427a3f2b89.jpg";
        const IMAGE_HASH: &str = "1aea8e98e0e5d969b7124f553b88dfae47d1f00472ea8c0dbf4ac4577d39ef02";
        let url = Url::parse(IMAGE_URL).unwrap();
        let hash = Sha256Hash::from_str(IMAGE_HASH).unwrap();
        let private_key = SecretKey::from_bech32(
            "nsec1qrypzwmxp8r54ctx2x7mhqzh5exca7xd8ssnlfup0js9l6pwku3qacq4u3",
        )
        .unwrap();
        let key = Keys::new(private_key);
        let signer = &key.into();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        let metadata = FileMetadata::new(url, "image/jpeg", hash);
        client.connect().await;
        let result = file_metadata(&client, signer, metadata, "Hello, world!").await;
        assert!(result.is_ok());
    }

    #[wasm_bindgen_test]
    async fn test_send_private_msg() {
        let private_key = SecretKey::from_bech32(
            "nsec1qrypzwmxp8r54ctx2x7mhqzh5exca7xd8ssnlfup0js9l6pwku3qacq4u3",
        )
        .unwrap();
        let key = Keys::new(private_key);
        let signer = &key.into();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        let receiver = PublicKey::from_bech32(
            "npub1awsnqr5338h497yam5m9hrgh9535yadj9zxglwk55xpsdtsn2c4syjruew",
        )
        .unwrap();
        client.connect().await;
        let result = send_private_msg(&client, signer, receiver, "Hello, world!", None).await;
        assert!(result.is_ok());
    }

    #[wasm_bindgen_test]
    async fn test_delete_event() {
        let private_key = SecretKey::from_bech32(
            "nsec1qrypzwmxp8r54ctx2x7mhqzh5exca7xd8ssnlfup0js9l6pwku3qacq4u3",
        )
        .unwrap();
        let key = Keys::new(private_key);
        let signer = &key.into();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        let event_id =
            EventId::from_bech32("note1zlsz37aggmsc2nfzqjdsdw77qwyfqm3erxag5f75nz8tndkvs0uqllhywm")
                .unwrap();
        client.connect().await;
        let result = delete_event(&client, signer, vec![event_id]).await;
        assert!(result.is_ok());
    }

    #[wasm_bindgen_test]
    async fn test_set_relay_list() {
        let private_key = SecretKey::from_bech32(
            "nsec1qrypzwmxp8r54ctx2x7mhqzh5exca7xd8ssnlfup0js9l6pwku3qacq4u3",
        )
        .unwrap();
        let key = Keys::new(private_key);
        let signer = &key.into();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        let relays = vec![
            (
                Url::parse("wss://relay.damus.io").unwrap(),
                Some(RelayMetadata::Read),
            ),
            (
                Url::parse("wss://relay.damus.io").unwrap(),
                Some(RelayMetadata::Write),
            ),
        ];
        client.connect().await;
        let result = set_relay_list(&client, signer, relays).await;
        assert!(result.is_ok());
    }

    #[wasm_bindgen_test]
    async fn test_set_contact_list() {
        let private_key = SecretKey::from_bech32(
            "nsec1qrypzwmxp8r54ctx2x7mhqzh5exca7xd8ssnlfup0js9l6pwku3qacq4u3",
        )
        .unwrap();
        let key = Keys::new(private_key);
        let signer = &key.into();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        let contacts = vec![Contact::new(
            PublicKey::from_bech32(
                "npub1awsnqr5338h497yam5m9hrgh9535yadj9zxglwk55xpsdtsn2c4syjruew",
            )
            .unwrap(),
            None,
            None::<&str>,
        )];
        client.connect().await;
        let result = set_contact_list(&client, signer, contacts).await;
        assert!(result.is_ok());
    }

    #[wasm_bindgen_test]
    async fn test_follow() {
        let private_key = SecretKey::from_bech32(
            "nsec1qt5ptz2rx83j5d758p72tqm2kh8w0gvq4l7ca9etk8v3n5zsxw5qw8f4y4",
        )
        .unwrap();
        let key = Keys::new(private_key);
        let signer = &key.into();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.add_relay("wss://nos.lol/").await.unwrap();
        let followee = PublicKey::from_bech32(
            "npub1awsnqr5338h497yam5m9hrgh9535yadj9zxglwk55xpsdtsn2c4syjruew",
        )
        .unwrap();
        client.connect().await;
        let result = follow(&client, signer, followee, None, None, None).await;
        assert!(result.is_ok());
    }

    #[wasm_bindgen_test]
    async fn test_unfollow() {
        let private_key = SecretKey::from_bech32(
            "nsec1qt5ptz2rx83j5d758p72tqm2kh8w0gvq4l7ca9etk8v3n5zsxw5qw8f4y4",
        )
        .unwrap();
        let key = Keys::new(private_key);
        let signer = &key.into();
        let client = Client::default();
        client.add_relay("wss://relay.damus.io").await.unwrap();
        client.add_relay("wss://nos.lol/").await.unwrap();
        let followee = PublicKey::from_bech32(
            "npub1awsnqr5338h497yam5m9hrgh9535yadj9zxglwk55xpsdtsn2c4syjruew",
        )
        .unwrap();
        client.connect().await;
        let result = unfollow(&client, signer, followee, None).await;
        assert!(result.is_ok());
    }
}
