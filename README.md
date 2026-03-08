# NexLink - AI 时代的智能代理应用

NexLink 是一个革命性的、AI 优先的分布式代理系统，结合了 P2P 网络技术和智能 AI 代理功能。它由三个核心组件构成：`app`、`relay` 和 `node`，提供智能路由、缓存、负载均衡等功能，以优化 AI 服务的访问和管理。

## 🌟 特性

### P2P 网络功能
- **去中心化架构**: 分布式节点无需中央服务器
- **智能发现**: 自动发现和连接网络中的节点
- **中继支持**: 通过中继服务器穿透 NAT
- **安全通信**: 端到端加密和身份验证

### AI 代理功能
- **智能路由**: 根据模型类型和端点性能自动路由请求
- **多提供商支持**: 统一访问 OpenAI、Anthropic、本地模型等
- **高效缓存**: 智能缓存 AI 响应，减少延迟和成本
- **负载均衡**: 多种策略优化性能和可用性
- **性能监控**: 实时监控和统计
- **OpenAI 兼容**: 与 OpenAI API 格式兼容

## 🏗️ 系统架构

### 组件
- **nexlink-app**: 终端用户 AI 代理应用程序
- **nexlink-relay**: P2P 中继和发现服务器
- **nexlink-node**: 智能 P2P 节点（可作为客户端或服务提供商）

详情请参见 [系统架构文档](SYSTEM_ARCHITECTURE.md)

## 🚀 快速开始

### 先决条件

- Rust 1.70+
- Cargo

### 构建

```bash
# 构建整个工作区
cargo build --workspace
```

### 部署示例

#### 1. 启动中继服务器
```bash
cd nexlink-relay
cargo run -- --listen /ip4/0.0.0.0/udp/4001/quic-v1
```

#### 2. 启动 AI 服务提供商节点
```bash
cd nexlink-node
cargo run -- --relay /ip4/127.0.0.1/udp/4001/quic-v1/p2p/<RELAY_PEER_ID> --ai-provider --ai-proxy-port 8080
```

#### 3. 启动 AI 代理应用
```bash
cd nexlink-app
cargo tauri dev
```

### 使用示例

#### 通过 AI 代理发送请求
```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4",
    "messages": [
      {
        "role": "user",
        "content": "你好！"
      }
    ],
    "temperature": 0.7
  }'
```

#### 检查系统状态
```bash
curl http://localhost:8080/health
curl http://localhost:8080/stats
```

## 🛠 技术栈

- **Rust**: 高性能和内存安全
- **Tokio**: 异步运行时
- **Libp2p**: P2P 网络协议
- **Axum**: HTTP 服务器框架
- **Serde**: 数据序列化
- **Tracing**: 应用程序内省

## 📊 核心功能

### 智能 AI 代理协调器
- **AI 服务管理**: 注册、管理和监控多个 AI 服务
- **智能路由**: 基于负载、响应时间和模型兼容性选择最佳端点
- **缓存系统**: 高效的响应缓存，减少重复请求
- **性能监控**: 实时收集和分析性能指标

### P2P 网络层
- **节点发现**: 自动发现网络中的 AI 服务提供商
- **连接管理**: 智能连接管理和故障恢复
- **代理路由**: 在节点间传输代理请求
- **安全通信**: 加密通信和身份验证

## 🎯 应用场景

1. **AI 服务聚合**: 统一管理多个 AI 服务提供商
2. **成本优化**: 通过缓存和负载均衡减少 API 调用
3. **高可用性**: 去中心化架构提供容错能力
4. **私有化部署**: 企业内部部署，数据不出网
5. **边缘计算**: 在靠近用户的地方部署 AI 服务

## 🔧 扩展性

该架构支持轻松扩展：
- 添加新的 AI 服务端点
- 集成更多模型提供商
- 实现高级缓存策略
- 扩展 P2P 网络功能

## 🤝 贡献

欢迎贡献！请遵循以下步骤：

1. Fork 仓库
2. 创建功能分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 打开 Pull Request

## 📄 许可证

此项目采用 MIT 许可证 - 详见 [LICENSE](./LICENSE) 文件。

## 📞 支持

如有问题，请提交 issue 或查看我们的文档。

---

<p align="center">
  为 AI 时代构建的分布式智能代理系统 🤖
</p>