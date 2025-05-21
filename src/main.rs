use nostr_crdt::nostr::crdt::{
    CrdtManager, CrdtOperation, CrdtState, GCounter, GSet, GSetAction, LWWRegister,
};
use nostr_crdt::nostr::fetch;
use nostr_indexeddb::nostr::nips::nip19::ToBech32;
use nostr_sdk::{Client, ClientBuilder, EventBuilder, Keys, Kind, SecretKey, Tag};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("CRDT Demo using Nostr");

    // 生成一个新的私钥，或者使用现有的私钥
    let secret_key = SecretKey::generate();
    println!("使用私钥: {}", secret_key.to_bech32().unwrap());

    let keys = Keys::new(secret_key);

    // 创建Nostr客户端
    let client = Client::new(&keys);

    // 添加一些中继服务器
    client.add_relay("wss://relay.damus.io").await?;
    client.add_relay("wss://nos.lol").await?;
    client.add_relay("wss://nostr.wine").await?;
    client.add_relay("wss://relay.nostr.band").await?;
    client.add_relay("wss://relay.snort.social").await?;

    // 连接到中继服务器
    client.connect().await;
    println!("已连接到中继服务器");

    // 等待连接确认
    tokio::time::sleep(Duration::from_secs(2)).await;

    // 获取签名器
    let signer = client.signer().await?;

    // 创建CRDT管理器
    let crdt_manager = CrdtManager::new(Arc::new(client.clone()), signer.clone(), keys.clone());

    // 1. 演示LWW-Register
    println!("\n演示 Last-Writer-Wins Register:");

    // 更新注册表
    let event_id = crdt_manager
        .update_lww_register("username", "capybara")
        .await?;
    println!("已发布用户名更新事件: {}", event_id);

    // 获取当前值
    let username = crdt_manager.get_register_value("username");
    println!("当前用户名: {:?}", username);

    // 更新为新值
    let event_id = crdt_manager
        .update_lww_register("username", "super_capybara")
        .await?;
    println!("已发布新的用户名更新事件: {}", event_id);

    // 获取更新后的值
    let username = crdt_manager.get_register_value("username");
    println!("更新后的用户名: {:?}", username);

    // 2. 演示G-Counter
    println!("\n演示 Grow-only Counter:");

    // 增加计数器
    let event_id = crdt_manager.increment_counter("visitors", 1).await?;
    println!("已发布访问者计数增加事件: {}", event_id);

    // 再次增加计数器
    let event_id = crdt_manager.increment_counter("visitors", 5).await?;
    println!("已发布访问者计数增加事件: {}", event_id);

    // 获取当前计数
    let visitors = crdt_manager.get_counter_value("visitors");
    println!("当前访问者数量: {:?}", visitors);

    // 3. 演示G-Set
    println!("\n演示 Grow-only Set:");

    // 添加到集合
    let event_id = crdt_manager.add_to_set("tags", "nostr").await?;
    println!("已发布标签添加事件: {}", event_id);

    // 添加更多到集合
    let event_id = crdt_manager.add_to_set("tags", "crdt").await?;
    println!("已发布标签添加事件: {}", event_id);

    let event_id = crdt_manager.add_to_set("tags", "distributed").await?;
    println!("已发布标签添加事件: {}", event_id);

    // 获取当前集合
    let tags = crdt_manager.get_set_value("tags");
    println!("当前标签集合: {:?}", tags);

    // 等待一段时间以确保事件已传播
    println!("\n等待事件传播...");
    tokio::time::sleep(Duration::from_secs(5)).await;

    // 创建一个过滤器来获取CRDT事件
    let filter = crdt_manager.get_filter();

    // 从网络获取事件
    println!("\n从网络获取CRDT事件:");

    // 添加错误处理
    let events = match client
        .get_events_of(vec![filter], Some(Duration::from_secs(10)))
        .await
    {
        Ok(events) => events,
        Err(err) => {
            println!("获取事件时出错: {}", err);
            Vec::new() // 返回空数组而不是报错
        }
    };

    println!("发现 {} 个CRDT事件", events.len());
    for (i, event) in events.iter().enumerate() {
        println!("事件 {}: ID: {}", i + 1, event.id);

        // 检查是否包含hashtag标记
        let is_crdt_event = event.tags.iter().any(|tag| {
            if let Some(values) = tag.as_vec().get(0..2) {
                return values.len() == 2 && values[0] == "t" && values[1] == "nostr-crdt";
            }
            false
        });

        if !is_crdt_event {
            println!("不是CRDT事件，跳过");
            continue;
        }

        // 检查内容是否需要解密
        let content = if event.content.contains("?iv=") {
            println!("内容已加密，尝试解密...");
            // 重要：使用发送者的公钥（event.pubkey）解密
            match signer.nip04_decrypt(event.pubkey, &event.content).await {
                Ok(decrypted) => {
                    println!("解密成功");
                    decrypted
                }
                Err(err) => {
                    println!("解密失败: {}", err);
                    event.content.clone()
                }
            }
        } else {
            event.content.clone()
        };

        println!("事件内容: {}", content);
        match serde_json::from_str::<CrdtOperation>(&content) {
            Ok(op) => println!("操作: {:?}", op),
            Err(err) => println!("无法解析操作: {}", err),
        }
    }

    println!("\nCRDT演示完成");

    // CRDT合并测试 - 模拟不同顺序接收操作后的最终一致性
    println!("\n===== CRDT合并测试 =====");

    println!("1. LWW-Register合并测试 (最后写入者获胜):");

    // 创建一个模拟的CRDT管理器，只做本地测试
    let mut lww_register = LWWRegister::default();

    // 较早的操作
    let op_a = CrdtOperation::LWWRegister {
        key: "test_key".to_string(),
        value: "值A".to_string(),
        timestamp: 100,
    };

    // 较晚的操作
    let op_b = CrdtOperation::LWWRegister {
        key: "test_key".to_string(),
        value: "值B".to_string(),
        timestamp: 200,
    };

    // 模拟设备1: 先应用A后应用B
    println!("  设备1: 先应用A(时间戳100)，后应用B(时间戳200)");
    let mut device1 = LWWRegister::default();
    device1.apply_operation(op_a.clone()).unwrap();
    println!("    应用A后值: {:?}", device1.get_value("test_key"));
    device1.apply_operation(op_b.clone()).unwrap();
    println!("    应用B后值: {:?}", device1.get_value("test_key"));

    // 模拟设备2: 先应用B后应用A
    println!("  设备2: 先应用B(时间戳200)，后应用A(时间戳100)");
    let mut device2 = LWWRegister::default();
    device2.apply_operation(op_b.clone()).unwrap();
    println!("    应用B后值: {:?}", device2.get_value("test_key"));
    device2.apply_operation(op_a.clone()).unwrap();
    println!("    应用A后值: {:?}", device2.get_value("test_key"));

    // 验证最终状态一致
    println!(
        "  最终结果: 设备1={:?}, 设备2={:?}",
        device1.get_value("test_key"),
        device2.get_value("test_key")
    );
    println!(
        "  合并成功: {}",
        device1.get_value("test_key") == device2.get_value("test_key")
    );

    // G-Counter合并测试
    println!("\n2. G-Counter合并测试 (只增计数器):");

    // 模拟设备1: 先+3后+2
    println!("  设备1: 先+3后+2");
    let mut counter1 = GCounter::default();
    counter1
        .apply_operation(CrdtOperation::GCounter {
            key: "test_counter".to_string(),
            increment: 3,
        })
        .unwrap();
    println!("    +3后计数: {:?}", counter1.get_value("test_counter"));

    counter1
        .apply_operation(CrdtOperation::GCounter {
            key: "test_counter".to_string(),
            increment: 2,
        })
        .unwrap();
    println!("    +2后计数: {:?}", counter1.get_value("test_counter"));

    // 模拟设备2: 先+2后+3
    println!("  设备2: 先+2后+3");
    let mut counter2 = GCounter::default();
    counter2
        .apply_operation(CrdtOperation::GCounter {
            key: "test_counter".to_string(),
            increment: 2,
        })
        .unwrap();
    println!("    +2后计数: {:?}", counter2.get_value("test_counter"));

    counter2
        .apply_operation(CrdtOperation::GCounter {
            key: "test_counter".to_string(),
            increment: 3,
        })
        .unwrap();
    println!("    +3后计数: {:?}", counter2.get_value("test_counter"));

    // 验证最终计数相同
    println!(
        "  最终结果: 设备1={:?}, 设备2={:?}",
        counter1.get_value("test_counter"),
        counter2.get_value("test_counter")
    );
    println!(
        "  合并成功: {}",
        counter1.get_value("test_counter") == counter2.get_value("test_counter")
    );

    // G-Set合并测试
    println!("\n3. G-Set合并测试 (只增集合):");

    // 模拟设备1: 添加 A、B、C
    println!("  设备1: 添加顺序 A->B->C");
    let mut set1 = GSet::default();
    set1.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "A".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    println!("    添加A后: {:?}", set1.get_value("test_set"));

    set1.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "B".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    println!("    添加B后: {:?}", set1.get_value("test_set"));

    set1.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "C".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    println!("    添加C后: {:?}", set1.get_value("test_set"));

    // 模拟设备2: 添加 C、A、B (不同顺序)
    println!("  设备2: 添加顺序 C->A->B");
    let mut set2 = GSet::default();
    set2.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "C".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    println!("    添加C后: {:?}", set2.get_value("test_set"));

    set2.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "A".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    println!("    添加A后: {:?}", set2.get_value("test_set"));

    set2.apply_operation(CrdtOperation::GSet {
        key: "test_set".to_string(),
        value: "B".to_string(),
        action: GSetAction::Add,
    })
    .unwrap();
    println!("    添加B后: {:?}", set2.get_value("test_set"));

    // 验证最终集合相同
    println!(
        "  最终结果: 设备1={:?}, 设备2={:?}",
        set1.get_value("test_set"),
        set2.get_value("test_set")
    );
    println!(
        "  合并成功: {}",
        set1.get_value("test_set") == set2.get_value("test_set")
    );

    println!("\nCRDT合并测试完成 - 展示了无论操作顺序如何，最终状态都会收敛到相同结果");

    // 断开连接
    client.disconnect().await?;

    Ok(())
}
