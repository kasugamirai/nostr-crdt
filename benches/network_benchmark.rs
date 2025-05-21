use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use futures::executor::block_on;
use nostr_crdt::nostr::crdt::{CrdtManager, CrdtOperation, GSetAction};
use nostr_sdk::{Client, EventBuilder, Keys, Kind, NostrSigner, SecretKey, Tag, TagKind};
use std::sync::Arc;
use tokio::runtime::Runtime;

// 创建模拟的客户端环境
fn setup_client() -> (Client, Keys, Arc<Client>, NostrSigner) {
    // 生成随机密钥
    let secret_key = SecretKey::generate();
    let keys = Keys::new(secret_key);

    // 创建客户端
    let client = Client::new(&keys);
    let arc_client = Arc::new(client.clone());
    let signer = NostrSigner::Keys(keys.clone());

    (client, keys, arc_client, signer)
}

// 基准测试CRDT操作发布性能
fn bench_publish_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("CRDT_Network_Operations");

    // 发布LWW-Register更新
    group.bench_function("publish_lww_register", |b| {
        b.iter_batched(
            || {
                // 设置
                let (_, keys, client, signer) = setup_client();
                let rt = Runtime::new().unwrap();
                let crdt_manager = rt.block_on(async { CrdtManager::new(client, signer, keys) });

                (rt, crdt_manager)
            },
            |(mut rt, crdt_manager)| {
                // 执行操作
                rt.block_on(async {
                    let _ = crdt_manager
                        .update_lww_register("benchmark_key", "benchmark_value")
                        .await;
                });
            },
            BatchSize::SmallInput,
        );
    });

    // 发布G-Counter增加
    group.bench_function("publish_g_counter", |b| {
        b.iter_batched(
            || {
                // 设置
                let (_, keys, client, signer) = setup_client();
                let rt = Runtime::new().unwrap();
                let crdt_manager = rt.block_on(async { CrdtManager::new(client, signer, keys) });

                (rt, crdt_manager)
            },
            |(mut rt, crdt_manager)| {
                // 执行操作
                rt.block_on(async {
                    let _ = crdt_manager.increment_counter("benchmark_counter", 1).await;
                });
            },
            BatchSize::SmallInput,
        );
    });

    // 发布G-Set添加
    group.bench_function("publish_g_set", |b| {
        b.iter_batched(
            || {
                // 设置
                let (_, keys, client, signer) = setup_client();
                let rt = Runtime::new().unwrap();
                let crdt_manager = rt.block_on(async { CrdtManager::new(client, signer, keys) });

                (rt, crdt_manager)
            },
            |(mut rt, crdt_manager)| {
                // 执行操作
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

// 基准测试处理事件性能
fn bench_process_events(c: &mut Criterion) {
    let mut group = c.benchmark_group("CRDT_Event_Processing");

    // 准备一个模拟事件
    let setup_event = || {
        let (_, keys, _, _) = setup_client();
        let rt = Runtime::new().unwrap();

        // 创建一个模拟的CRDT操作
        let op = CrdtOperation::LWWRegister {
            key: "test_key".to_string(),
            value: "test_value".to_string(),
            timestamp: 12345,
        };

        // 序列化操作
        let content = serde_json::to_string(&op).unwrap();

        // 创建Nostr事件
        let mut event_builder = EventBuilder::new(
            Kind::TextNote,
            &content,
            vec![
                Tag::hashtag("nostr-crdt"),
                Tag::custom(TagKind::from("c"), ["crdt", "lww"]),
            ],
        );

        // 设置时间戳
        event_builder = event_builder.custom_created_at(nostr_sdk::Timestamp::now());

        // 签名事件
        let event = rt.block_on(async { event_builder.to_event(&keys).unwrap() });

        event
    };

    // 处理LWW-Register事件
    group.bench_function("process_lww_event", |b| {
        b.iter_batched(
            || {
                // 设置
                let (_, keys, client, signer) = setup_client();
                let rt = Runtime::new().unwrap();
                let crdt_manager = rt.block_on(async { CrdtManager::new(client, signer, keys) });
                let event = setup_event();

                (rt, crdt_manager, event)
            },
            |(mut rt, crdt_manager, event)| {
                // 执行操作
                rt.block_on(async {
                    let _ = crdt_manager.process_event(&event).await;
                });
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

// 定义并运行基准测试组
criterion_group!(benches, bench_publish_operations, bench_process_events);
criterion_main!(benches);
