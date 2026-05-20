# Copi

[中文文档](README_CN.md) | [Usage examples](USAGE_EXAMPLES.md)

Copi is a Go-based clipboard sync project. The product goal is simple: copy on one device, paste on the others.

This repository has been rewritten from the old Rust implementation. The current codebase contains the new Go CLI core, a third-party HTTP relay service, the device-side client sync loop, and zero-config LAN mode. Native clients should be thin shells around the CLI: settings, tray/menu-bar UI, startup integration, notifications, and packaging.

## Current Status

- Go core: implemented
- CLI as the cross-platform sync engine: implemented
- Server mode: implemented as a third-party HTTP relay that never reads or writes the machine's clipboard
- LAN mode: implemented
- Text clipboard sync: implemented
- Image, file, and rich text sync: protocol space reserved
- Native clients: planned

## Modes

### Server Mode

Use this when devices sync through a central service running on a third-party machine. The server is only an HTTP relay: it does not need a GUI and never touches the local clipboard. Real copy/paste behavior happens on clients, which only need the relay URL.

```bash
go run ./cmd/copi server --addr 0.0.0.0:9527 --token your-secret
```

Connect a client:

```bash
go run ./cmd/copi client --server http://192.168.1.10:9527 --token your-secret
```

### LAN Mode

Use this when devices are on the same Wi-Fi or local network. Each device runs LAN mode, discovers peers over UDP multicast, and syncs directly.

```bash
go run ./cmd/copi lan --token your-secret
```

If the auto-detected address is wrong, set the advertised URL manually:

```bash
go run ./cmd/copi lan --listen 0.0.0.0:9528 --advertise http://192.168.1.20:9528 --token your-secret
```

## Build

```bash
go build -o bin/copi ./cmd/copi
```

## One-Command Docker Relay

Run the third-party HTTP relay on a server:

```bash
printf "COPI_TOKEN=%s\n" "$(openssl rand -hex 16)" > .env && docker compose up -d
```

The relay exposes port `9527` by default. Clients connect to that address:

```bash
copi client --server http://SERVER_IP:9527 --token YOUR_TOKEN
```

See [docs/DOCKER.md](docs/DOCKER.md) for more Docker options.

Run tests:

```bash
go test ./...
```

## Commands

```text
copi server [--addr 0.0.0.0:9527] [--token secret]
copi relay [--addr 0.0.0.0:9527] [--token secret]
copi client --server http://host:9527 [--token secret]
copi lan [--listen 0.0.0.0:9528] [--token secret]
copi status [--json]
copi version
```

Config commands:

```bash
copi config init
copi config show
copi config path
```

Long-running commands support structured logs for native shells:

```bash
copi client --log-format json
copi lan --log-format json
copi relay --log-format json
```

See [docs/CLI_CONTRACT.md](docs/CLI_CONTRACT.md) for the shell-facing CLI contract.

## Native Client Direction

The Go CLI core owns protocol, sync, discovery, and server behavior. Native clients should first wrap the CLI instead of reimplementing sync. Use `copi status --json` for machine-readable feature detection; do not parse human logs as an API.

- macOS: SwiftUI + NSPasteboard
- Windows: WinUI 3 + C#/.NET is the recommended native stack; WPF + .NET is also practical for tray/background-first apps
- Linux: GTK/libadwaita or Qt with Wayland/X11 clipboard backends
- iOS/iPadOS: SwiftUI + UIPasteboard
- Android: Kotlin + Jetpack Compose + ClipboardManager

## Architecture

```text
cmd/copi/                 CLI entrypoint and cross-platform sync engine
internal/protocol/        Clipboard message protocol
internal/server/          Third-party HTTP relay service
internal/client/          Server-mode client sync loop
internal/lan/             LAN discovery and peer sync
internal/clipboard/       System clipboard adapters
internal/config/          Device ID and local config
```

Protocol endpoints:

- `POST /v1/clipboard` publishes clipboard content
- `GET /v1/clipboard?since=<seq>&wait=30s` long-polls for the newest clipboard content
- `GET /health` checks service health

## Security

Copi currently supports an optional shared token. Clipboard data is still sent over plain HTTP, so use it on trusted networks or servers you control. TLS, pairing codes, device authorization, and end-to-end encryption are good next steps.

## License

MIT License
