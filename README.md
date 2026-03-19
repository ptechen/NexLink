# NexLink

NexLink 是一个基于 **Rust + libp2p + Tauri + Leptos** 的 P2P 代理/中继项目，包含三类核心组件：

- **nexlink-relay**：中继 / rendezvous 服务端
- **nexlink-node**：节点程序，可作为客户端或 provider
- **nexlink-app**：桌面 GUI（Tauri + Leptos）
- **nexlink-taos**：TDengine/taos 接入层，为后续流量数据写入做准备

这份 README 的目标是让你能快速完成下面几件事：

- 本地开发
- 构建二进制
- 启动 relay
- 启动 node
- 构建桌面应用
- 使用一键部署脚本

---

## 目录结构

```text
.
├── crates/                # 共享库
├── nexlink-relay/         # Relay 服务端
├── nexlink-node/          # Node 客户端 / Provider
├── nexlink-app/           # Tauri + Leptos 桌面端
├── deploy.sh              # 一键部署脚本
└── README.md
```

---

## 技术栈

- **Rust workspace**
- **libp2p**：P2P 网络、relay、rendezvous、ping、identify
- **Tokio**：异步运行时
- **Tauri 2**：桌面应用壳
- **Leptos**：前端 UI
- **Trunk**：WASM 前端构建
- **Tailwind CSS**：样式构建

---

## 环境要求

### 必需

- Rust（建议稳定版）
- Cargo
- Git

### 如果你要构建桌面 App

还需要：

- `trunk`
- Node.js / npm
- Tauri 对应平台依赖

macOS 上常见安装方式：

```bash
brew install node
cargo install trunk
cargo install tauri-cli
```

> 如果 `cargo install` 因环境问题失败，请先确认 Rust toolchain 正常：
>
> ```bash
> rustup show
> cargo --version
> rustc --version
> ```

---

## 快速开始

### 1. 克隆仓库

```bash
git clone git@github.com:ptechen/NexLink.git
cd NexLink
```

### 2. 一键检查环境

```bash
./deploy.sh doctor
```

### 3. 构建核心二进制

```bash
./deploy.sh build
```

构建完成后输出位于：

```text
target/release/nexlink-relay
target/release/nexlink-node
```

---

## Relay 启动

Relay 需要一个凭据密钥，用于给 client/provider 派发代理认证信息。

### 方式一：通过环境变量

```bash
export NEXLINK_CREDENTIALS_SECRET="replace-with-a-long-random-secret"
./deploy.sh relay
```

### 方式二：通过命令行参数

```bash
./deploy.sh relay --secret "replace-with-a-long-random-secret"
```

### 自定义监听地址

```bash
./deploy.sh relay --listen "/ip4/0.0.0.0/udp/4001/quic-v1" --secret "replace-with-a-long-random-secret"
```

Relay 启动后会打印类似地址：

```text
/ip4/1.2.3.4/udp/4001/quic-v1/p2p/<PEER_ID>
```

后续 node 连接 relay 时就用这个完整地址。

---

## Node 启动

Node 需要知道 relay 地址。

### 客户端模式

```bash
./deploy.sh node --relay "/ip4/127.0.0.1/udp/4001/quic-v1/p2p/<PEER_ID>"
```

默认会在本地启动统一代理端口：

- `7890`

### Provider 模式

```bash
./deploy.sh node --relay "/ip4/127.0.0.1/udp/4001/quic-v1/p2p/<PEER_ID>" --provider
```

### 指定命名空间

```bash
./deploy.sh node --relay "/ip4/127.0.0.1/udp/4001/quic-v1/p2p/<PEER_ID>" --namespace "nexlink-public"
```

### 指定本地统一代理端口

```bash
./deploy.sh node --relay "/ip4/127.0.0.1/udp/4001/quic-v1/p2p/<PEER_ID>" --port 7891
```

---

## 桌面 App 开发

进入前端目录：

```bash
cd nexlink-app
```

### 安装前端依赖

```bash
npm install
```

### 构建 Tailwind 样式

```bash
npm run build:tailwind
```

### 启动前端开发服务器

```bash
trunk serve
```

### 启动 Tauri 桌面开发

```bash
cargo tauri dev
```

也可以直接用脚本：

```bash
./deploy.sh app-dev
```

---

## 构建桌面 App

```bash
./deploy.sh app-build
```

如果环境完整，产物会由 Tauri 输出到对应构建目录。

---

## 一键部署脚本说明

项目根目录提供 `deploy.sh`，支持以下命令：

```bash
./deploy.sh doctor       # 检查环境
./deploy.sh build        # 构建 relay + node
./deploy.sh relay        # 启动 relay
./deploy.sh node         # 启动 node
./deploy.sh app-dev      # 启动桌面开发模式
./deploy.sh app-build    # 构建桌面应用
./deploy.sh help         # 查看帮助
```

### 示例

#### 构建后启动 relay

```bash
./deploy.sh build
./deploy.sh relay --secret "replace-with-a-long-random-secret"
```

#### 启动 provider 节点

```bash
./deploy.sh node \
  --relay "/ip4/127.0.0.1/udp/4001/quic-v1/p2p/<PEER_ID>" \
  --provider
```

#### 启动普通 client 节点

```bash
./deploy.sh node \
  --relay "/ip4/127.0.0.1/udp/4001/quic-v1/p2p/<PEER_ID>" \
  --port 7890
```

---

## 常见问题

### 1. relay 启动时报错：missing proxy credentials secret

说明你没有提供：

- `--credentials-secret`
- 或 `NEXLINK_CREDENTIALS_SECRET`

修复：

```bash
export NEXLINK_CREDENTIALS_SECRET="replace-with-a-long-random-secret"
./deploy.sh relay
```

### 2. node 连不上 relay

先检查 relay 地址是否包含：

```text
/p2p/<PEER_ID>
```

这是必须的。

### 3. 桌面 app 构建失败

优先检查：

- `node -v`
- `npm -v`
- `trunk --version`
- `cargo tauri --version`

以及系统是否装齐 Tauri 平台依赖。

### 4. Tailwind 没编译

手动执行：

```bash
cd nexlink-app
npm install
npm run build:tailwind
```

---

## 推荐开发流程

### 后端 / 网络部分

```bash
./deploy.sh doctor
./deploy.sh build
./deploy.sh relay --secret "replace-with-a-long-random-secret"
./deploy.sh node --relay "/ip4/127.0.0.1/udp/4001/quic-v1/p2p/<PEER_ID>"
```

### GUI 部分

```bash
./deploy.sh app-dev
```

---

## TDengine / taos 支持（进行中）

当前 `crates/nexlink-taos` 已提供第一版基础设施：

- `TaosConfig`：读取 DSN / database / stable 配置
- `TaosClient`：建立连接并确保 traffic schema 存在
- `TrafficWriteRepository`：预留流量样本写入接口与批量 flush helper
- `TrafficSample`：统一流量写入模型（含 `source_ip` / `source_transport`）

默认读取以下环境变量：

```bash
export NEXLINK_TAOS_DSN="taos+ws://localhost:6041"
export NEXLINK_TAOS_DATABASE="nexlink"
export NEXLINK_TAOS_STABLE="traffic_metrics"
```

这部分目前是**为未来流量落库打地基**，还没有把 runtime 中的真实流量采集自动接入写库链路。

## 后续可以继续补的内容

如果你要继续完善这个项目，建议下一步做这些：

1. 增加 `.env` / `config` 示例文件
2. 补 docker / docker-compose 部署
3. 增加 systemd 服务文件
4. 增加 CI（GitHub Actions）自动构建
5. 在 README 里补一张架构图

---

## License

项目当前包含 `LICENSE` 文件，默认按仓库中的许可证执行。
