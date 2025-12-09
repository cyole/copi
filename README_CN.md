# Copi - 跨平台剪贴板同步工具

[![CI](https://github.com/cyole/copi/workflows/CI/badge.svg)](https://github.com/cyole/copi/actions)
[![Release](https://github.com/cyole/copi/workflows/Release/badge.svg)](https://github.com/cyole/copi/releases)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[English](README.md) | [使用示例](USAGE_EXAMPLES.md)

一个跨 Linux 和 macOS 系统的剪贴板同步工具，使用 Rust 编写。

## 功能特性

- ✨ 跨平台支持（Linux 和 macOS）
- 📝 支持文本和图片剪贴板同步
- 🖼️ 自动检测并同步图片（PNG 格式）
- 🔄 实时剪贴板监控
- 🌐 网络同步剪贴板内容
- 🚀 轻量级和高性能
- 🔒 使用 SHA-256 避免重复同步
- 🎯 完整支持 Wayland（使用 wl-clipboard）

## 系统要求

- Rust 1.70 或更高版本
- Linux 或 macOS 操作系统

> **注意**：Windows 平台尚未测试。虽然代码可能可以在 Windows 上编译，但剪贴板功能和网络同步尚未在该平台上验证。

### Linux 系统依赖

在 Linux 上，需要安装 X11 或 Wayland 的剪贴板支持：

**对于 X11：**
```bash
# Ubuntu/Debian
sudo apt-get install libxcb-shape0-dev libxcb-xfixes0-dev

# Fedora
sudo dnf install libxcb-devel
```

**对于 Wayland（推荐）：**
```bash
# Ubuntu/Debian
sudo apt install wl-clipboard

# Fedora
sudo dnf install wl-clipboard

# Arch Linux
sudo pacman -S wl-clipboard
```

程序会自动检测运行环境（X11 或 Wayland）并使用相应的剪贴板后端。

## 安装

### 方式 1: 从 GitHub Release 下载（推荐）

从 [Releases 页面](https://github.com/cyole/copi/releases) 下载适合你系统的预编译二进制文件：

```bash
# 下载（以 Linux x86_64 为例，请根据你的系统选择）
wget https://github.com/cyole/copi/releases/latest/download/copi-Linux-x86_64.tar.gz

# 解压
tar xzf copi-Linux-x86_64.tar.gz

# 移动到系统路径（可选）
sudo mv copi /usr/local/bin/

# 验证安装
copi --help
```

可用的平台：
- `copi-Linux-x86_64.tar.gz` - Linux x86_64
- `copi-Linux-aarch64.tar.gz` - Linux ARM64
- `copi-Darwin-x86_64.tar.gz` - macOS Intel
- `copi-Darwin-aarch64.tar.gz` - macOS Apple Silicon

### 方式 2: 从源码编译

```bash
# 克隆仓库
git clone https://github.com/cyole/copi
cd copi

# 编译项目
cargo build --release

# 可执行文件位于
./target/release/copi

# 可选：安装到系统路径
cargo install --path .
```

## 使用方法

### 服务器模式

在一台机器上启动服务器：

```bash
./target/release/copi server
# 或者在开发时
cargo run -- server
```

默认监听地址为 `0.0.0.0:9527`。你也可以指定自定义地址：

```bash
copi server --addr 0.0.0.0:8080
```

**只转发模式**（适用于无图形界面的服务器）：

如果你需要在没有图形界面或剪贴板访问权限的服务器上运行，可以使用 `--relay-only` 参数。在这种模式下，服务器只转发客户端之间的剪贴板数据，不会尝试访问本地剪贴板：

```bash
copi server --relay-only
# 或指定地址
copi server --addr 0.0.0.0:8080 --relay-only
```

这种模式特别适合用于云服务器、Docker 容器或其他无头环境。

### 客户端模式

在另一台机器上启动客户端：

```bash
copi client --server <服务器IP>:9527
```

例如：

```bash
copi client --server 192.168.1.100:9527
```

客户端会自动监听本地剪贴板变化（包括文本和图片），并与服务器同步。

### 支持的剪贴板内容

- ✅ 纯文本
- ✅ 图片（PNG、JPEG 等格式，内部转换为 PNG）
- ⏳ 未来可能支持：文件、富文本等

## 工作原理

1. **服务器端**：
   - 监听指定端口接收客户端连接
   - 监控本地剪贴板变化
   - 接收来自客户端的剪贴板内容

2. **客户端端**：
   - 连接到服务器
   - 监控本地剪贴板变化并发送到服务器
   - 接收服务器推送的剪贴板内容
   - 自动更新本地剪贴板

3. **去重机制**：
   - 使用 SHA-256 哈希值跟踪剪贴板内容
   - 避免相同内容的重复同步

## 架构

```
src/
├── main.rs                 # 主程序入口和 CLI 处理
└── modules/
    ├── mod.rs             # 模块声明
    ├── clipboard.rs       # 剪贴板监控模块
    └── sync.rs            # 网络同步模块
```

## 依赖项

- `arboard` - 跨平台剪贴板访问（支持文本和图片）
- `tokio` - 异步运行时
- `serde` / `serde_json` - 序列化和反序列化
- `anyhow` - 错误处理
- `clap` - 命令行参数解析
- `sha2` - SHA-256 哈希计算
- `base64` - 图片数据编码
- `image` - 图片处理和格式转换

## 安全注意事项

- 目前的实现使用明文传输剪贴板内容
- 建议在受信任的网络环境中使用
- 未来版本可以添加 TLS/SSL 加密支持

## 许可证

MIT License

## 贡献

欢迎提交 Issue 和 Pull Request！
