# Copi

[English](README.md) | [使用示例](USAGE_EXAMPLES.md)

Copi 是一个用 Go 重构的剪贴板同步项目。新目标很简单：一台设备复制，其他设备可以粘贴。

这次重构已经抛弃旧 Rust 逻辑，当前仓库包含新的 Go CLI 核心、第三方 HTTP 中转服务、客户端同步循环和局域网自动发现模式。原生桌面/移动客户端只需要围绕 CLI 做一个壳：设置页、托盘/菜单栏、开机启动、通知和打包。

## 当前状态

- Go 核心：已迁移
- CLI 作为跨端同步内核：已实现
- 服务器模式：已实现，服务端是第三方 HTTP 中转服务，不读取也不写入本机剪贴板
- 局域网模式：已实现
- 剪贴板文本同步：已实现
- 图片、文件、富文本同步：协议已预留，后续实现
- 原生客户端：规划中

## 模式

### 服务器模式

适合跨网络、多设备、长期在线同步。这里的“服务器”是跑在第三方机器上的 HTTP 中转服务，机器本身不需要图形界面，也不会访问剪贴板。所有真正复制/粘贴的设备都是客户端，只要在客户端里填这个服务地址即可使用。

```bash
go run ./cmd/copi server --addr 0.0.0.0:9527 --token your-secret
```

客户端连接这个第三方服务：

```bash
go run ./cmd/copi client --server http://192.168.1.10:9527 --token your-secret
```

### 局域网模式

适合同一个 Wi-Fi 或同一个局域网内的设备。每台设备都运行 LAN 模式，设备会自动发现彼此并直接同步。

```bash
go run ./cmd/copi lan --token your-secret
```

如果自动识别的本机地址不对，可以手动指定对外广播地址：

```bash
go run ./cmd/copi lan --listen 0.0.0.0:9528 --advertise http://192.168.1.20:9528 --token your-secret
```

## 构建

```bash
go build -o bin/copi ./cmd/copi
```

## Docker 一键部署第三方服务

在第三方机器上运行 HTTP 中转服务：

```bash
printf "COPI_TOKEN=%s\n" "$(openssl rand -hex 16)" > .env && docker compose up -d
```

服务默认暴露 `9527` 端口。客户端填写这个地址即可：

```bash
copi client --server http://服务器IP:9527 --token 你的TOKEN
```

更多 Docker 配置见 [docs/DOCKER.md](docs/DOCKER.md)。

运行测试：

```bash
go test ./...
```

## 命令

```text
copi server [--addr 0.0.0.0:9527] [--token secret]
copi relay [--addr 0.0.0.0:9527] [--token secret]
copi client --server http://host:9527 [--token secret]
copi lan [--listen 0.0.0.0:9528] [--token secret]
copi status [--json]
copi version
```

配置文件入口：

```bash
copi config init
copi config show
copi config path
```

长运行命令支持结构化日志，给原生壳监听：

```bash
copi client --log-format json
copi lan --log-format json
copi relay --log-format json
```

CLI 契约见 [docs/CLI_CONTRACT.md](docs/CLI_CONTRACT.md)。

## 原生客户端方向

Go CLI 核心负责协议、同步、发现和服务端能力；各端原生客户端优先只做原生壳，启动并管理 CLI 进程。原生壳不要解析人类日志，如果需要机器可读信息，使用 `copi status --json`，后续再补更多 JSON 控制命令。

- macOS：SwiftUI + NSPasteboard
- Windows：推荐 WinUI 3 + C#/.NET；如果要更贴近系统托盘和后台服务，也可以用 WPF + .NET
- Linux：GTK/libadwaita 或 Qt，剪贴板走 Wayland/X11 后端
- iOS/iPadOS：SwiftUI + UIPasteboard
- Android：Kotlin + Jetpack Compose + ClipboardManager

Windows 如果你还没定技术栈，我建议先选 WinUI 3 + C#/.NET：足够原生，生态稳定，也方便做托盘、开机启动、设置页和通知。

## 架构

```text
cmd/copi/                 CLI 入口，跨端同步内核
internal/protocol/        剪贴板消息协议
internal/server/          第三方 HTTP 中转服务
internal/client/          服务器模式客户端同步循环
internal/lan/             局域网发现和点对点同步
internal/clipboard/       系统剪贴板适配
internal/config/          设备 ID 和本地配置
```

协议入口：

- `POST /v1/clipboard` 发布剪贴板
- `GET /v1/clipboard?since=<seq>&wait=30s` 长轮询获取最新剪贴板
- `GET /health` 健康检查

## 安全

当前支持可选共享 token。同步内容仍是明文 HTTP，适合可信局域网或你自己控制的服务器环境。后续可以加 TLS、配对码、设备授权和端到端加密。

## 许可证

MIT License
