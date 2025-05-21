use nostr_crdt::nostr::crdt::{CrdtManager, CrdtOperation};
use nostr_crdt::nostr::fetch;
use nostr_indexeddb::nostr::nips::nip19::ToBech32;
use nostr_sdk::{Client, ClientBuilder, EventBuilder, Keys, Kind, SecretKey, Tag};
use std::sync::Arc;
use std::time::Duration;
use wasm_bindgen_futures::spawn_local;

/// Demonstrates the correct way to encrypt and decrypt messages with NIP-04
async fn encryption_decryption_example(
    client: &Client,
    my_keys: &Keys,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n演示 NIP-04 加密和解密:");

    // 获取我们自己的密钥
    let my_signer = client.signer().await?;
    let my_pubkey = my_signer.public_key().await?;

    // 模拟另一个用户（在实际使用中，这可能是其他人的公钥）
    let recipient_secret_key = SecretKey::generate();
    let recipient_keys = Keys::new(recipient_secret_key);
    let recipient_client = Client::new(&recipient_keys);
    recipient_client.add_relay("wss://relay.damus.io").await?;
    recipient_client.connect().await;

    let recipient_signer = recipient_client.signer().await?;
    let recipient_pubkey = recipient_signer.public_key().await?;

    println!("我的公钥: {}", my_pubkey.to_bech32()?);
    println!("接收者公钥: {}", recipient_pubkey.to_bech32()?);

    // 1. 发送加密消息
    // 重要：使用接收者的公钥加密
    let encrypted_content = my_signer
        .nip04_encrypt(recipient_pubkey, "秘密消息")
        .await?;

    // 创建加密的直接消息事件
    let tags = vec![Tag::public_key(recipient_pubkey)];
    let event = EventBuilder::new(Kind::EncryptedDirectMessage, &encrypted_content, tags)
        .to_event(my_keys)?;

    // 发送事件
    client.send_event(event.clone()).await?;
    println!("已发送加密消息: {}", event.id);

    // 等待确保消息已传输
    tokio::time::sleep(Duration::from_secs(1)).await;

    // 2. 接收者解密消息
    // 重要：接收者使用发送者的公钥（而不是自己的公钥）来解密
    let decrypted_content = recipient_signer
        .nip04_decrypt(event.pubkey, &event.content)
        .await?;
    println!("接收者解密后的内容: {}", decrypted_content);

    // 3. 回复消息
    let reply_content = recipient_signer
        .nip04_encrypt(my_pubkey, "回复消息")
        .await?;

    let tags = vec![Tag::public_key(my_pubkey)];
    let reply_event = EventBuilder::new(Kind::EncryptedDirectMessage, &reply_content, tags)
        .to_event(&recipient_keys)?;

    recipient_client.send_event(reply_event.clone()).await?;
    println!("接收者已回复加密消息: {}", reply_event.id);

    // 等待确保消息已传输
    tokio::time::sleep(Duration::from_secs(1)).await;

    // 4. 我解密回复消息
    // 重要：使用发送者（接收者）的公钥进行解密
    let decrypted_reply = my_signer
        .nip04_decrypt(recipient_pubkey, &reply_event.content)
        .await?;
    println!("我解密后的回复内容: {}", decrypted_reply);

    Ok(())
}

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

    // 运行加密解密示例
    encryption_decryption_example(&client, &keys).await?;

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

    // 断开连接
    client.disconnect().await?;

    Ok(())
}
