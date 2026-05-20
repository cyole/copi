# Copi

[中文文档](README_CN.md) | [Usage examples](USAGE_EXAMPLES.md)

Copi is a Go CLI core for clipboard sync. The product goal is simple: copy on one device, paste on the others.

The CLI has clear boundaries:

- `copi relay` runs the third-party HTTP relay service.
- `copi client --relay ...` runs a real device client through that relay.
- `copi client --lan` runs a real device client with LAN discovery.
- `copi doctor` diagnoses setup.
- `copi version` prints version and capabilities.

## Relay Mode

Run the third-party relay on a server. This process never reads or writes the server machine's clipboard.

```bash
copi relay --addr 0.0.0.0:9527 --token your-secret
```

Run a device client:

```bash
copi client --relay http://192.168.1.10:9527 --token your-secret
```

## LAN Mode

Run this on each device in the same LAN:

```bash
copi client --lan --token your-secret
```

If auto-detected addressing is wrong:

```bash
copi client --lan \
  --listen 0.0.0.0:9528 \
  --advertise http://192.168.1.20:9528 \
  --token your-secret
```

## Docker Relay

Run the third-party relay with Docker:

```bash
printf "COPI_TOKEN=%s\n" "$(openssl rand -hex 16)" > .env && docker compose up -d
```

Then connect clients:

```bash
copi client --relay http://SERVER_IP:9527 --token YOUR_TOKEN
```

See [docs/DOCKER.md](docs/DOCKER.md) for more Docker options.

## Doctor

```bash
copi doctor --json
copi doctor --mode client --relay http://SERVER_IP:9527 --json
copi doctor --mode lan --json
copi doctor --mode relay --json
```

## Build

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

See [docs/CLI_CONTRACT.md](docs/CLI_CONTRACT.md) for the shell-facing CLI contract.

## Native Client Direction

Native apps should be thin shells around the CLI: settings, tray/menu-bar UI, startup integration, notifications, and packaging. Use `copi version --json` for machine-readable capabilities and JSON logs for long-running process state.

Recommended native stacks:

- macOS: SwiftUI + NSPasteboard
- Windows: WinUI 3 + C#/.NET
- Linux: GTK/libadwaita or Qt with Wayland/X11 clipboard backends
- iOS/iPadOS: SwiftUI + UIPasteboard
- Android: Kotlin + Jetpack Compose + ClipboardManager

## License

MIT License
