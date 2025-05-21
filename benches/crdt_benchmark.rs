use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nostr_crdt::nostr::crdt::{CrdtOperation, CrdtState, GCounter, GSet, GSetAction, LWWRegister};
use nostr_sdk::{Event, EventBuilder, Keys, Kind, NostrSigner, Tag, Timestamp};
use std::sync::Arc;

// 基准测试LWW-Register性能
fn bench_lww_register(c: &mut Criterion) {
    let mut group = c.benchmark_group("LWWRegister");

    // 单个更新操作性能
    group.bench_function("single_update", |b| {
        b.iter(|| {
            let mut lww = LWWRegister::default();
            lww.apply_operation(CrdtOperation::LWWRegister {
                key: "test_key".to_string(),
                value: "test_value".to_string(),
                timestamp: 100,
            })
            .unwrap();
        });
    });

    // 冲突解决性能 (较新的时间戳获胜)
    group.bench_function("conflict_resolution_newer_wins", |b| {
        b.iter(|| {
            let mut lww = LWWRegister::default();
            // 先应用较早的操作
            lww.apply_operation(CrdtOperation::LWWRegister {
                key: "test_key".to_string(),
                value: "older_value".to_string(),
                timestamp: 100,
            })
            .unwrap();

            // 再应用较晚的操作
            lww.apply_operation(CrdtOperation::LWWRegister {
                key: "test_key".to_string(),
                value: "newer_value".to_string(),
                timestamp: 200,
            })
            .unwrap();
        });
    });

    // 冲突解决性能 (忽略较旧的操作)
    group.bench_function("conflict_resolution_ignore_older", |b| {
        b.iter(|| {
            let mut lww = LWWRegister::default();
            // 先应用较晚的操作
            lww.apply_operation(CrdtOperation::LWWRegister {
                key: "test_key".to_string(),
                value: "newer_value".to_string(),
                timestamp: 200,
            })
            .unwrap();

            // 再应用较早的操作 (应该被忽略)
            lww.apply_operation(CrdtOperation::LWWRegister {
                key: "test_key".to_string(),
                value: "older_value".to_string(),
                timestamp: 100,
            })
            .unwrap();
        });
    });

    // 批量操作性能测试
    let batch_sizes = [10, 100, 1000];
    for size in batch_sizes {
        group.bench_with_input(
            BenchmarkId::new("batch_updates", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let mut lww = LWWRegister::default();
                    for i in 0..size {
                        lww.apply_operation(CrdtOperation::LWWRegister {
                            key: format!("key_{}", i),
                            value: format!("value_{}", i),
                            timestamp: i as u64,
                        })
                        .unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// 基准测试G-Counter性能
fn bench_g_counter(c: &mut Criterion) {
    let mut group = c.benchmark_group("GCounter");

    // 单个增加操作性能
    group.bench_function("single_increment", |b| {
        b.iter(|| {
            let mut counter = GCounter::default();
            counter
                .apply_operation(CrdtOperation::GCounter {
                    key: "visitors".to_string(),
                    increment: 1,
                })
                .unwrap();
        });
    });

    // 多次增加同一计数器
    group.bench_function("multiple_increments_same_counter", |b| {
        b.iter(|| {
            let mut counter = GCounter::default();
            for _ in 0..100 {
                counter
                    .apply_operation(CrdtOperation::GCounter {
                        key: "visitors".to_string(),
                        increment: 1,
                    })
                    .unwrap();
            }
        });
    });

    // 增加多个不同计数器
    group.bench_function("multiple_counters", |b| {
        b.iter(|| {
            let mut counter = GCounter::default();
            for i in 0..100 {
                counter
                    .apply_operation(CrdtOperation::GCounter {
                        key: format!("counter_{}", i),
                        increment: 1,
                    })
                    .unwrap();
            }
        });
    });

    group.finish();
}

// 基准测试G-Set性能
fn bench_g_set(c: &mut Criterion) {
    let mut group = c.benchmark_group("GSet");

    // 单个添加元素操作性能
    group.bench_function("single_add", |b| {
        b.iter(|| {
            let mut set = GSet::default();
            set.apply_operation(CrdtOperation::GSet {
                key: "tags".to_string(),
                value: "tag1".to_string(),
                action: GSetAction::Add,
            })
            .unwrap();
        });
    });

    // 添加多个元素到同一集合
    group.bench_function("add_multiple_elements", |b| {
        b.iter(|| {
            let mut set = GSet::default();
            for i in 0..100 {
                set.apply_operation(CrdtOperation::GSet {
                    key: "tags".to_string(),
                    value: format!("tag_{}", i),
                    action: GSetAction::Add,
                })
                .unwrap();
            }
        });
    });

    // 测试元素添加的幂等性 (重复添加相同元素)
    group.bench_function("idempotent_adds", |b| {
        b.iter(|| {
            let mut set = GSet::default();
            for _ in 0..10 {
                // 重复添加相同的10个元素
                for i in 0..10 {
                    set.apply_operation(CrdtOperation::GSet {
                        key: "tags".to_string(),
                        value: format!("tag_{}", i),
                        action: GSetAction::Add,
                    })
                    .unwrap();
                }
            }
        });
    });

    group.finish();
}

// 基准测试序列化和反序列化性能
fn bench_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("Serialization");

    // LWW操作序列化
    let lww_op = CrdtOperation::LWWRegister {
        key: "username".to_string(),
        value: "capybara".to_string(),
        timestamp: 12345678,
    };

    group.bench_function("serialize_lww", |b| {
        b.iter(|| {
            black_box(serde_json::to_string(&lww_op).unwrap());
        });
    });

    let lww_json = serde_json::to_string(&lww_op).unwrap();
    group.bench_function("deserialize_lww", |b| {
        b.iter(|| {
            let _: CrdtOperation = black_box(serde_json::from_str(&lww_json).unwrap());
        });
    });

    // GCounter操作序列化
    let counter_op = CrdtOperation::GCounter {
        key: "visitors".to_string(),
        increment: 42,
    };

    group.bench_function("serialize_counter", |b| {
        b.iter(|| {
            black_box(serde_json::to_string(&counter_op).unwrap());
        });
    });

    let counter_json = serde_json::to_string(&counter_op).unwrap();
    group.bench_function("deserialize_counter", |b| {
        b.iter(|| {
            let _: CrdtOperation = black_box(serde_json::from_str(&counter_json).unwrap());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_lww_register,
    bench_g_counter,
    bench_g_set,
    bench_serialization,
);
criterion_main!(benches);
