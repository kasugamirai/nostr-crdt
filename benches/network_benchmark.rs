use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use nostr_crdt::nostr::crdt::{CrdtManager, CrdtOperation};
use nostr_sdk::{Client, EventBuilder, Keys, Kind, NostrSigner, SecretKey, Tag, TagKind};
use std::sync::Arc;
use tokio::runtime::Runtime;

fn setup_client() -> (Client, Keys, Arc<Client>, NostrSigner) {
    let secret_key = SecretKey::generate();
    let keys = Keys::new(secret_key);
    let client = Client::new(&keys);
    let arc_client = Arc::new(client.clone());
    let signer = NostrSigner::Keys(keys.clone());

    (client, keys, arc_client, signer)
}

fn bench_publish_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("CRDT_Network_Operations");

    group.bench_function("publish_lww_register", |b| {
        b.iter_batched(
            || {
                let (_, keys, client, signer) = setup_client();
                let rt = Runtime::new().unwrap();
                let crdt_manager = rt.block_on(async { CrdtManager::new(client, signer, keys) });

                (rt, crdt_manager)
            },
            |(rt, crdt_manager)| {
                rt.block_on(async {
                    let _ = crdt_manager
                        .update_lww_register("benchmark_key", "benchmark_value")
                        .await;
                });
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("publish_g_counter", |b| {
        b.iter_batched(
            || {
                let (_, keys, client, signer) = setup_client();
                let rt = Runtime::new().unwrap();
                let crdt_manager = rt.block_on(async { CrdtManager::new(client, signer, keys) });

                (rt, crdt_manager)
            },
            |(rt, crdt_manager)| {
                rt.block_on(async {
                    let _ = crdt_manager.increment_counter("benchmark_counter", 1).await;
                });
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("publish_g_set", |b| {
        b.iter_batched(
            || {
                let (_, keys, client, signer) = setup_client();
                let rt = Runtime::new().unwrap();
                let crdt_manager = rt.block_on(async { CrdtManager::new(client, signer, keys) });

                (rt, crdt_manager)
            },
            |(rt, crdt_manager)| {
                rt.block_on(async {
                    let _ = crdt_manager
                        .add_to_set("benchmark_set", "benchmark_item")
                        .await;
                });
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_process_events(c: &mut Criterion) {
    let mut group = c.benchmark_group("CRDT_Event_Processing");

    let setup_event = || {
        let (_, keys, _, _) = setup_client();
        let rt = Runtime::new().unwrap();

        let op = CrdtOperation::LWWRegister {
            key: "test_key".to_string(),
            value: "test_value".to_string(),
            timestamp: 12345,
        };

        let content = serde_json::to_string(&op).unwrap();

        let mut event_builder = EventBuilder::new(
            Kind::TextNote,
            &content,
            vec![
                Tag::hashtag("nostr-crdt"),
                Tag::custom(TagKind::from("c"), ["crdt", "lww"]),
            ],
        );

        event_builder = event_builder.custom_created_at(nostr_sdk::Timestamp::now());

        rt.block_on(async { event_builder.to_event(&keys).unwrap() })
    };

    group.bench_function("process_lww_event", |b| {
        b.iter_batched(
            || {
                let (_, keys, client, signer) = setup_client();
                let rt = Runtime::new().unwrap();
                let crdt_manager = rt.block_on(async { CrdtManager::new(client, signer, keys) });
                let event = setup_event();

                (rt, crdt_manager, event)
            },
            |(rt, crdt_manager, event)| {
                rt.block_on(async {
                    let _ = crdt_manager.process_event(&event).await;
                });
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_publish_operations, bench_process_events);
criterion_main!(benches);
