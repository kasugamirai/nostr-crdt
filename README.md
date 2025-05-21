# Nostr-CRDT

一个基于Nostr协议实现的CRDT文本编辑器同步系统，可以实现去中心化的实时协作文本编辑。

## 功能特点

- 基于Nostr协议的去中心化通信
- 使用CRDT算法解决冲突
- 支持实时文本编辑同步
- 支持离线编辑，在重新连接后自动合并更改

## 技术栈

- Rust语言实现
- Nostr-SDK用于去中心化通信
- Yrs (Yjs的Rust实现) 提供CRDT数据结构

## 项目状态

**注意**: 此项目目前处于开发中，有一些已知的限制：

1. Yrs (CRDT库) 的API与我们的实现存在一些不匹配
2. Nostr-SDK的某些API用法需要更新
3. TextEditor需要实现克隆功能

## 使用方法

### 安装

```bash
git clone https://github.com/yourusername/nostr-crdt.git
cd nostr-crdt
cargo build
```

### 运行

```bash
cargo run
```

## 项目结构

- `src/crdt.rs` - CRDT核心实现，使用Yrs库
- `src/editor.rs` - 文本编辑器实现
- `src/sync.rs` - Nostr同步管理器，使用Nostr-SDK
- `src/types.rs` - 数据类型定义

## 如何贡献

该项目需要以下改进:

1. 修复Yrs库的使用方式，确保正确导入traits
2. 更新Nostr-SDK的API使用
3. 改进错误处理
4. 添加用户界面组件

欢迎提交Pull Request或Issue!

## 许可证

MIT 