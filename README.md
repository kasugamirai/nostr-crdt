# Nostr-CRDT

使用Nostr实现的CRDT（冲突解决数据类型）库，支持加密数据同步。

## 特性

- 支持三种基本CRDT类型：
  - LWW-Register（最后写入者获胜）
  - G-Counter（只增计数器）
  - G-Set（只增集合）
- 使用Nostr网络作为传输层
- 支持NIP-04加密
- 可靠的冲突解决方案
- 无需中央服务器的分布式数据同步

## 安装

将依赖添加到您的Cargo.toml：

```toml
[dependencies]
nostr-crdt = "0.1.0"
```

## 基本用法

```rust
use nostr_crdt::nostr::crdt::{CrdtManager, CrdtOperation};
use nostr_sdk::{Client, Keys, SecretKey};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建Nostr客户端
    let secret_key = SecretKey::generate();
    let keys = Keys::new(secret_key);
    let client = Client::new(&keys);
    
    // 添加中继
    client.add_relay("wss://relay.damus.io").await?;
    client.connect().await;
    
    // 创建CRDT管理器
    let signer = client.signer().await?;
    let crdt_manager = CrdtManager::new(Arc::new(client.clone()), signer.clone(), keys.clone());
    
    // 更新LWW-Register
    crdt_manager.update_lww_register("username", "capybara").await?;
    
    // 增加计数器
    crdt_manager.increment_counter("visitors", 1).await?;
    
    // 添加到集合
    crdt_manager.add_to_set("tags", "nostr").await?;
    
    Ok(())
}
```

## 性能测试

该项目包含全面的基准测试套件，用于测量CRDT操作的性能。

运行基准测试：

```bash
cargo bench
```

基准测试包括：

1. **CRDT操作性能**
   - LWW-Register更新和冲突解决
   - G-Counter递增
   - G-Set添加和幂等性

2. **序列化/反序列化性能**
   - 不同CRDT类型的JSON序列化/反序列化

3. **加密/解密性能**
   - NIP-04加密/解密操作

4. **网络操作性能**
   - CRDT操作的发布和处理

## 原理

CRDT（冲突解决数据类型）是一种特殊数据结构，允许分布式系统中的节点独立修改数据，且能自动合并这些修改而不产生冲突。

在该实现中：

1. 每个CRDT操作被序列化为JSON
2. 使用NIP-04加密（仅限发起者可以解密）
3. 通过Nostr网络传输
4. 接收方解密并应用操作

最重要的特性是：无论操作以何种顺序到达，系统最终都会达到一致状态。

## 许可证

MIT 