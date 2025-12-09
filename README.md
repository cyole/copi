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

## System Requirements

- Rust 1.70 or higher
- Linux or macOS operating system

> **Note**: Windows support has not been tested. While the code may compile on Windows, clipboard functionality and network synchronization have not been verified on this platform.

### Linux System Dependencies

On Linux, you need to install clipboard support for either X11 or Wayland:

**For X11:**
```bash
# Ubuntu/Debian
sudo apt-get install libxcb-shape0-dev libxcb-xfixes0-dev

# Fedora
sudo dnf install libxcb-devel
```

**For Wayland (Recommended):**
```bash
# Ubuntu/Debian
sudo apt install wl-clipboard

# Fedora
sudo dnf install wl-clipboard

# Arch Linux
sudo pacman -S wl-clipboard
```

The program automatically detects the running environment (X11 or Wayland) and uses the appropriate clipboard backend.

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

### Client Mode

Start the client on another machine:

```bash
copi client --server <server-ip>:9527
```

For example:

```bash
copi client --server 192.168.1.100:9527
```

The client automatically monitors local clipboard changes (including text and images) and syncs with the server.

### Supported Clipboard Content

- ✅ Plain text
- ✅ Images (PNG, JPEG, and other formats, internally converted to PNG)
- ⏳ Future support may include: files, rich text, etc.

## How It Works

1. **Server Side**:
   - Listens on a specified port for client connections
   - Monitors local clipboard changes
   - Receives clipboard content from clients

2. **Client Side**:
   - Connects to the server
   - Monitors local clipboard changes and sends them to the server
   - Receives clipboard content pushed by the server
   - Automatically updates the local clipboard

3. **Deduplication Mechanism**:
   - Uses SHA-256 hash values to track clipboard content
   - Avoids redundant synchronization of identical content

## Architecture

```
src/
├── main.rs                 # Main program entry and CLI handling
└── modules/
    ├── mod.rs             # Module declarations
    ├── clipboard.rs       # Clipboard monitoring module
    └── sync.rs            # Network synchronization module
```

## Dependencies

- `arboard` - Cross-platform clipboard access (supports text and images)
- `tokio` - Async runtime
- `serde` / `serde_json` - Serialization and deserialization
- `anyhow` - Error handling
- `clap` - Command-line argument parsing
- `sha2` - SHA-256 hash computation
- `base64` - Image data encoding
- `image` - Image processing and format conversion

## Security Considerations

- The current implementation transmits clipboard content in plain text
- Recommended for use in trusted network environments
- Future versions may add TLS/SSL encryption support

## License

MIT License

## Contributing

Issues and Pull Requests are welcome!

