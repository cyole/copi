# Copi

[English](README.md) | [使用示例](USAGE_EXAMPLES.md)

Copi 是一个用 Go 写的剪贴板同步 CLI 内核。目标很简单：一台设备复制，其他设备可以粘贴。

当前命令边界：

- `copi relay` 跑第三方 HTTP 中转服务。
- `copi client --relay ...` 让真实设备通过中转服务同步。
- `copi client --lan` 让真实设备通过局域网自动发现同步。
- `copi doctor` 做诊断。
- `copi version` 查看版本和能力。

## 中转模式

在第三方机器上运行 relay。这个进程不会读取或写入服务器机器的剪贴板。

```bash
copi relay --addr 0.0.0.0:9527 --token your-secret
```

真实设备连接 relay：

```bash
copi client --relay http://192.168.1.10:9527 --token your-secret
```

## 局域网模式

同一个局域网里的每台设备运行：

```bash
copi client --lan --token your-secret
```

如果自动识别的本机地址不对：

```bash
copi client --lan \
  --listen 0.0.0.0:9528 \
  --advertise http://192.168.1.20:9528 \
  --token your-secret
```

## Docker 部署 relay

```bash
printf "COPI_TOKEN=%s\n" "$(openssl rand -hex 16)" > .env && docker compose up -d
```

客户端连接：

```bash
copi client --relay http://服务器IP:9527 --token 你的TOKEN
```

更多 Docker 配置见 [docs/DOCKER.md](docs/DOCKER.md)。

## 诊断

```bash
copi doctor --json
copi doctor --mode client --relay http://服务器IP:9527 --json
copi doctor --mode lan --json
copi doctor --mode relay --json
```

## 构建

```bash
go build -o bin/copi ./cmd/copi
go test ./...
```

## CLI

```text
copi relay [--addr 0.0.0.0:9527] [--token secret] [--log-format text|json]
copi client --relay http://host:9527 [--token secret] [--log-format text|json]
copi client --lan [--listen 0.0.0.0:9528] [--token secret] [--log-format text|json]
copi doctor [--json] [--mode all|relay|client|lan]
copi version [--json]
```

CLI 契约见 [docs/CLI_CONTRACT.md](docs/CLI_CONTRACT.md)。

## 原生客户端方向

原生应用只做 CLI 的壳：设置页、托盘/菜单栏、开机启动、通知和打包。用 `copi version --json` 读取机器可读能力，用 JSON 日志监听长运行进程状态。

推荐技术栈：

- macOS：SwiftUI + NSPasteboard
- Windows：WinUI 3 + C#/.NET
- Linux：GTK/libadwaita 或 Qt，剪贴板走 Wayland/X11 后端
- iOS/iPadOS：SwiftUI + UIPasteboard
- Android：Kotlin + Jetpack Compose + ClipboardManager

## 许可证

MIT License
