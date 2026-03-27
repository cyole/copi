# Copi - Cross-Platform Clipboard Sync Tool

[![CI](https://github.com/cyole/copi/workflows/CI/badge.svg)](https://github.com/cyole/copi/actions)
[![Release](https://github.com/cyole/copi/workflows/Release/badge.svg)](https://github.com/cyole/copi/releases)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[中文文档](README_CN.md) | [使用示例](USAGE_EXAMPLES.md)

A cross-platform clipboard synchronization tool for Linux and macOS, written in Rust.

## Features

- ✨ Cross-platform support (Linux and macOS)
- 📝 Supports text and image clipboard synchronization
- 🖼️ Automatic detection and syncing of images (PNG format)
- 🔄 Real-time clipboard monitoring
- 🌐 Network-based clipboard synchronization
- 🚀 Lightweight and high-performance
- 🔒 Uses SHA-256 to avoid duplicate synchronization
- 🎯 Full Wayland support (using wl-clipboard)
- 🔑 Token-based authentication (HMAC-SHA256 challenge-response, token never sent over the wire)
- 👥 Multi-user support — each token = separate private clipboard group on the same server
- 🛡️ Server secret gate — prevents unauthorized access to internet-facing servers
- 🚫 Rate limiting — 10 failed auth attempts per IP per 10 minutes, then auto-blocked
- 🔐 TLS encryption (auto-generated self-signed certs or bring your own)
- 🏠 P2P LAN mode — automatic direct connection between clients on the same network
- 📡 LAN discovery fallback — UDP broadcast finds peers when server is unreachable
- 🐳 Docker support for headless relay servers

## System Requirements

- Rust 1.70 or higher
- Linux or macOS operating system

> **Note**: Windows support has not been tested. While the code may compile on Windows, clipboard functionality and network synchronization have not been verified on this platform.

### Linux System Dependencies

**Build dependencies (X11 libs):**
```bash
# Ubuntu/Debian
sudo apt-get install libxcb-shape0-dev libxcb-xfixes0-dev

# Fedora
sudo dnf install libxcb-devel

# Arch Linux
sudo pacman -S libxcb
```

**Runtime dependencies:**

| Package | Required for | Install |
|---|---|---|
| `xclip` | File copy/paste on GNOME Wayland | `sudo pacman -S xclip` / `sudo apt install xclip` |
| `wl-clipboard` | Clipboard on non-GNOME Wayland (Sway, Hyprland) | `sudo pacman -S wl-clipboard` / `sudo apt install wl-clipboard` |

**Clipboard backend selection (automatic):**

| Desktop | Backend | Text/Image | File copy/paste |
|---|---|---|---|
| GNOME (Wayland) | arboard + xclip | arboard (native, no flicker) | xclip via XWayland |
| Sway, Hyprland, etc. | wl-clipboard | wl-paste / wl-copy | wl-paste / wl-copy |
| macOS | arboard + osascript | arboard (native) | osascript (Finder integration) |

On GNOME, `wl-clipboard` is intentionally avoided because it creates popup windows that steal focus and cause dashboard flickering (GNOME lacks the data-control protocol). Instead, arboard accesses the Wayland clipboard natively, and xclip handles file URIs through XWayland.

## Installation

### Option 1: Download from GitHub Releases (Recommended)

Download pre-built binaries for your system from the [Releases page](https://github.com/cyole/copi/releases):

```bash
# Download (using Linux x86_64 as example, choose based on your system)
wget https://github.com/cyole/copi/releases/latest/download/copi-Linux-x86_64.tar.gz

# Extract
tar xzf copi-Linux-x86_64.tar.gz

# Move to system path (optional)
sudo mv copi /usr/local/bin/

# Verify installation
copi --help
```

Available platforms:
- `copi-Linux-x86_64.tar.gz` - Linux x86_64
- `copi-Linux-aarch64.tar.gz` - Linux ARM64
- `copi-Darwin-x86_64.tar.gz` - macOS Intel
- `copi-Darwin-aarch64.tar.gz` - macOS Apple Silicon

### Option 2: Build from Source

```bash
# Clone the repository
git clone https://github.com/cyole/copi
cd copi

# Build the project
cargo build --release

# The executable is located at
./target/release/copi

# Optional: Install to system path
cargo install --path .
```

## Usage

### Server Mode

Start the server on one machine:

```bash
./target/release/copi server
# Or during development
cargo run -- server
```

The default listening address is `0.0.0.0:9527`. You can also specify a custom address:

```bash
copi server --addr 0.0.0.0:8080
```

**Relay-Only Mode** (for headless servers):

If you need to run the server on a machine without a graphical interface or clipboard access (such as cloud servers or Docker containers), you can use the `--relay-only` flag. In this mode, the server only relays clipboard data between clients without attempting to access the local clipboard:

```bash
copi server --relay-only
# Or with custom address
copi server --addr 0.0.0.0:8080 --relay-only
```

This mode is particularly useful for cloud servers, Docker containers, or other headless environments.

**Token Authentication:**

You can require clients to authenticate with a token before syncing. The token is never sent over the network — instead, an HMAC-SHA256 challenge-response protocol is used:

1. Server sends a random nonce to the client
2. Client computes `HMAC-SHA256(token, nonce)` and sends back the hash
3. Server verifies using constant-time comparison

This prevents eavesdropping and replay attacks even without TLS.

The server supports two modes:

**Single-group mode** (`--token` set on server):
```bash
copi server --token my-secret-token
```
All clients must provide the same token. Server validates the HMAC. One shared clipboard.

**Multi-group mode** (no `--token` on server):
```bash
copi server --relay-only --tls-auto-cert
```
Any client can connect with any token. Clients with the **same token** share a clipboard group; different tokens are isolated. One server, unlimited private clipboard groups. The token acts as a room key — only people who know it can join that group.

**Server Secret (gate authentication):**

For internet-facing servers, set a server secret to prevent unauthorized access. Only clients with the matching secret can connect:

```bash
# Server
copi server --relay-only --secret my-server-secret --tls-auto-cert

# Or via environment variable
export COPI_SECRET=my-server-secret
copi server --relay-only --tls-auto-cert
```

Clients must provide the same secret:

```bash
copi client --server your-server.example.com --token my-token --secret my-server-secret --tls-skip-verify
```

The secret is validated via HMAC challenge-response (never sent in plaintext). Failed attempts are rate-limited: after 10 failures from the same IP within 10 minutes, all connections from that IP are dropped immediately.

**TLS Encryption:**

Enable TLS to encrypt all traffic (clipboard data, images, auth handshake):

```bash
# Auto-generate a self-signed certificate (development/testing)
copi server --tls-auto-cert

# Use your own certificate files
copi server --cert /path/to/cert.pem --key /path/to/key.pem

# Combine with token authentication
copi server --token my-secret-token --tls-auto-cert
```

TLS is optional. Without TLS flags, the server runs plain TCP (backward-compatible).

### Client Mode

Start the client on another machine:

```bash
copi client --server <server-ip>:9527
```

For example:

```bash
copi client --server 192.168.1.100:9527
```

If the server requires token authentication, pass the same token:

```bash
copi client --server 192.168.1.100:9527 --token my-secret-token

# Or via environment variable
COPI_TOKEN=my-secret-token copi client --server 192.168.1.100:9527
```

If the server has TLS enabled, the client must also enable TLS:

```bash
# With a CA certificate for verification
copi client --server 192.168.1.100:9527 --ca-cert /path/to/ca.pem

# Skip certificate verification (for self-signed certs)
copi client --server 192.168.1.100:9527 --tls-skip-verify

# Full setup: TLS + token + server secret
copi client --server 192.168.1.100:9527 --token my-secret-token --secret my-server-secret --tls-skip-verify
```

The client automatically monitors local clipboard changes (including text and images) and syncs with the server.

### File Sync

Sync a directory across all connected peers using `--sync-dir`. Files in the directory are monitored for changes and automatically synced. Use `--max-file-size` to limit the maximum file size (in MB, default 10 MB).

```bash
# On each machine, point --sync-dir to the folder you want to keep in sync
copi client --server 192.168.1.100:9527 --sync-dir ~/shared-files

# Limit to files under 5 MB
copi client --server 192.168.1.100:9527 --sync-dir ~/shared-files --max-file-size 5

# Server in relay-only mode can also have a sync directory
copi server --relay-only --sync-dir /data/shared
```

Subdirectories are synced recursively. Files are scanned every second and only transferred when content changes (SHA-256 deduplication).

### Clipboard File Copy

Copy files with Ctrl+C (or Cmd+C) on one machine, paste with Ctrl+V (or Cmd+V) on another — completely transparent. No sync folders needed. Works cross-platform between Linux and macOS.

| Platform | Detection | Paste |
|---|---|---|
| **GNOME (Wayland)** | `xclip` reads `text/uri-list` via XWayland | `xclip` sets `text/uri-list` |
| **Sway, Hyprland** | `wl-paste --list-types` / `wl-paste --type text/uri-list` | `wl-copy --type text/uri-list` |
| **macOS** | `osascript clipboard info` for `«class furl»` | `osascript set the clipboard to POSIX file` |

When you copy a file, copi reads its content, sends it over the network, writes it to a temp directory on the other machine, and sets the clipboard so paste works natively in any file manager (Nautilus, Finder, Dolphin, etc.).

The `--max-file-size` flag controls the maximum size for clipboard file transfers (default 10 MB). Requires `xclip` on GNOME or `wl-clipboard` on other Wayland compositors.

### Supported Content

- ✅ Plain text clipboard
- ✅ Images (PNG, JPEG, and other formats, internally converted to PNG)
- ✅ Native file copy/paste (Ctrl+C / Cmd+C → Ctrl+V / Cmd+V across machines, Linux Wayland + macOS)
- ✅ Directory sync via `--sync-dir` (any file type, configurable size limit)

## How It Works

### Connection Modes

Copi uses a hybrid architecture — clients connect to a central relay server but automatically establish direct P2P connections when possible:

```
              ┌──────────────────────────┐
              │     Internet Server      │
              │  (relay, multi-group)    │
              └───┬─────────┬────────┬──┘
                  │         │        │
            ┌─────┴───┐ ┌──┴─────┐ ┌┴──────────┐
            │Client A │ │Client B│ │ Client C   │
            │ Linux   │ │ macOS  │ │  laptop    │
            └────┬────┘ └───┬────┘ │  (remote)  │
                 │          │      └────────────┘
                 └──P2P ────┘
                  (same LAN)
```

**Server mode (always active):**
- Clients connect to the relay server via TLS
- Server broadcasts clipboard changes to all clients in the same group
- Works across any network — home, office, mobile

**P2P mode (automatic when possible):**
- Server detects clients with the same public IP (behind same NAT)
- Sends `PeerDiscovery` with each peer's local LAN IP and listen port
- Clients connect directly over LAN (TCP on `--listen` port, default 9528)
- Lower latency, no server bandwidth used, 100 MB file limit (vs 10 MB through server)

**LAN discovery fallback (when server is down):**
- After 2 failed server connection attempts, clients broadcast UDP discovery packets on port 9529
- Other clients with matching token respond and establish direct P2P connections
- Server retry slows to every 30 seconds (server is source of truth when available)
- When server comes back, clients reconnect and resume normal mode

**Message deduplication:**
- When both P2P and server are active, the same message may arrive via both paths
- Router deduplicates by `(client_id, timestamp)` — second arrival is silently dropped

### Data Flow

1. Client monitors local clipboard every 500ms (SHA-256 dedup prevents re-sending unchanged content)
2. On change, content is sent to **all active connections** (server + P2P peers)
3. Other clients receive and update their local clipboard
4. Files copied via Ctrl+C/Cmd+C are read, encoded, sent, and written to temp dir on the receiving end — Ctrl+V pastes natively

## Performance

Copi is designed to be lightweight. Real-world measurements:

### Client (Linux, Arch, GNOME Wayland)

| Metric | Value |
|---|---|
| RSS (resident memory) | ~11 MB |
| Memory (systemd reported) | ~18 MB |
| Peak memory | ~21 MB |
| CPU usage (idle) | 0.0% |
| CPU time (26 min uptime) | ~7s total |
| Binary size | 16 MB (14 MB stripped) |

### Server (Docker, ARM64, Oracle Cloud)

| Metric | Value |
|---|---|
| RSS (resident memory) | ~7.5 MB |
| Virtual memory | ~80 MB |
| CPU usage (idle) | 0.00% |
| Threads | 2 |
| Docker image size | 113 MB |
| Network I/O (16 min) | ~20 KB in/out |

### Client (macOS, Apple Silicon, launchd)

| Metric | Value |
|---|---|
| RSS (resident memory) | ~20 MB |
| Virtual memory | ~422 MB (normal for macOS) |
| CPU usage (idle) | 0.0% |
| CPU time (17 min uptime) | ~6s total |
| Threads | 10 |
| Binary size | 8.7 MB (7.4 MB stripped) |

The client polls the clipboard every 500ms but only transfers data when content changes (SHA-256 deduplication). At idle, CPU usage is effectively zero. Memory footprint stays under 20 MB on both client and server.

## Architecture

```
src/
├── main.rs                 # CLI, connection loop, P2P listener, message routing
└── modules/
    ├── mod.rs             # Module declarations
    ├── clipboard.rs       # Clipboard monitoring (arboard/wl-clipboard/xclip/osascript)
    ├── discovery.rs       # UDP LAN peer discovery (fallback when server is down)
    ├── files.rs           # Directory sync file monitoring
    ├── sync.rs            # TCP protocol, server, client, auth, P2P peer tracking
    └── tls.rs             # TLS configuration and certificate handling
```

## Dependencies

- `arboard` - Cross-platform clipboard access (supports text and images)
- `tokio` - Async runtime
- `serde` / `serde_json` - Serialization and deserialization
- `anyhow` - Error handling
- `clap` - Command-line argument parsing
- `sha2` / `hmac` - HMAC-SHA256 authentication
- `tokio-rustls` / `rustls` - TLS encryption
- `rcgen` - Self-signed certificate generation
- `base64` - Image data encoding
- `image` - Image processing and format conversion

### Docker

Run the server as a Docker container with TLS and token authentication:

```bash
# Build the image
docker build -t copi-server .

# Multi-group mode (recommended) — each token = private clipboard group
docker run -d -p 9527:9527 \
  -e COPI_SECRET=your-server-secret \
  copi-server server --relay-only --addr 0.0.0.0:9527 --tls-auto-cert

# Single-group mode — one shared clipboard, server validates token
docker run -d -p 9527:9527 -e COPI_TOKEN=my-secret-token copi-server
```

Then connect clients. Each user picks their own token — users with the same token share a clipboard:

```bash
# User A's machines (share clipboard with each other)
copi client --server your-server.example.com --token user-a-secret --secret your-server-secret --tls-skip-verify

# User B's machines (separate clipboard, same server)
copi client --server your-server.example.com --token user-b-secret --secret your-server-secret --tls-skip-verify
```

**Complete production example (multi-group + secret + TLS):**

```bash
# Generate secrets
SERVER_SECRET=$(openssl rand -hex 24)
echo "Server secret: $SERVER_SECRET"

# Start server
docker run -d --name copi-server --restart unless-stopped \
  -p 9527:9527 \
  -e COPI_SECRET=$SERVER_SECRET \
  copi-server server --relay-only --addr 0.0.0.0:9527 --tls-auto-cert

# Connect clients (each user picks their own token for their private clipboard)
copi client --server your-server.example.com \
  --token my-clipboard-key \
  --secret $SERVER_SECRET \
  --tls-skip-verify
```

### Arch Linux (systemd user service)

Install required packages:

```bash
# Runtime: pick one based on your desktop
sudo pacman -S xclip          # GNOME Wayland — file copy/paste via XWayland
sudo pacman -S wl-clipboard   # Sway, Hyprland — clipboard access via wl-clipboard

# Build dependencies
sudo pacman -S rust libxcb
```

Build and install:

```bash
cargo build --release
mkdir -p ~/.local/bin
cp target/release/copi ~/.local/bin/
```

Create `~/.config/systemd/user/copi.service`:

```ini
[Unit]
Description=Copi - Clipboard Sync Client
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
Environment=COPI_TOKEN=your-secret-token
Environment=COPI_SECRET=your-server-secret
ExecStart=%h/.local/bin/copi client --server your-server.example.com --tls-skip-verify
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
```

Enable and start:

```bash
systemctl --user daemon-reload
systemctl --user enable copi.service
systemctl --user start copi.service

# Check status
systemctl --user status copi.service

# View logs
journalctl --user -u copi.service -f
```

### macOS (launchd user agent)

Install the binary and set up a service that starts at login:

```bash
# Build and install
cargo build --release
mkdir -p ~/.local/bin
cp target/release/copi ~/.local/bin/
```

Create `~/Library/LaunchAgents/com.copi.client.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.copi.client</string>
    <key>ProgramArguments</key>
    <array>
        <string>/Users/YOUR_USERNAME/.local/bin/copi</string>
        <string>client</string>
        <string>--server</string>
        <string>your-server.example.com</string>
        <string>--token</string>
        <string>your-secret-token</string>
        <string>--secret</string>
        <string>your-server-secret</string>
        <string>--tls-skip-verify</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/copi-client.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/copi-client.err</string>
</dict>
</plist>
```

Enable and start:

```bash
# Load the service (starts immediately and on every login)
launchctl load ~/Library/LaunchAgents/com.copi.client.plist

# Check status
launchctl list | grep copi

# View logs
tail -f /tmp/copi-client.log

# Stop the service
launchctl unload ~/Library/LaunchAgents/com.copi.client.plist
```

No extra dependencies required — macOS provides native clipboard access and Finder file detection out of the box.

## Security Considerations

- **Token authentication** uses HMAC-SHA256 challenge-response — the token is never transmitted over the network, preventing eavesdropping and replay attacks
- **Server secret** (`--secret` / `COPI_SECRET`) — gate authentication, required to connect at all. Prevents unauthorized use of your server. Validated via HMAC (never sent in plaintext).
- **Rate limiting** — 10 failed authentication attempts per IP per 10 minutes. After that, connections from that IP are dropped immediately.
- **TLS encryption** protects all traffic (clipboard content, images, auth handshake) from interception
- For maximum security, use TLS + server secret + token together
- The `--tls-skip-verify` flag disables certificate verification and should only be used with self-signed certs in trusted environments
- For production deployments, use proper CA-signed certificates with `--cert`/`--key` on the server and `--ca-cert` on clients

## License

MIT License

## Contributing

Issues and Pull Requests are welcome!

