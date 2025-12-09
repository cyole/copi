# 使用示例

## 场景 1: 在本地网络中同步两台机器的剪贴板

### 机器 A (服务器 - macOS)

```bash
# 启动服务器，监听所有网络接口
copi server

# 或者在开发时
cargo run -- server

# 或者指定特定端口
copi server --addr 0.0.0.0:8888
```

### 机器 B (客户端 - Linux)

```bash
# 连接到机器 A (假设机器 A 的 IP 是 192.168.1.100)
copi client --server 192.168.1.100:9527
```

现在，当你在机器 A 或机器 B 上复制任何文本或图片时，它会自动同步到另一台机器的剪贴板。

## 场景 2: 多客户端同步

你可以将多个客户端连接到同一个服务器：

### 服务器 (机器 A)
```bash
cargo run -- server
```

### 客户端 1 (机器 B)
```bash
cargo run -- client --server 192.168.1.100:9527
```

### 客户端 2 (机器 C)
```bash
cargo run -- client --server 192.168.1.100:9527
```

所有三台机器的剪贴板将保持同步。

## 场景 3: 自定义端口

如果默认端口已被占用：

### 服务器
```bash
cargo run -- server --addr 0.0.0.0:7777
```

### 客户端
```bash
cargo run -- client --server 192.168.1.100:7777 --listen 0.0.0.0:7778
```

## 测试

### 在 macOS 上测试
```bash
# 终端 1: 启动服务器
cargo run -- server

# 终端 2: 启动客户端 (连接到本地服务器)
cargo run -- client --server 127.0.0.1:9527

# 终端 3: 测试复制
echo "Hello from macOS" | pbcopy
```

### 在 Linux 上测试

**Wayland 环境：**
```bash
# 终端 1: 启动服务器
cargo run -- server

# 终端 2: 启动客户端
cargo run -- client --server 127.0.0.1:9527

# 终端 3: 测试复制 (使用 wl-clipboard)
echo "Hello from Wayland" | wl-copy

# 查看剪贴板内容
wl-paste
```

**X11 环境：**
```bash
# 终端 1: 启动服务器
cargo run -- server

# 终端 2: 启动客户端
cargo run -- client --server 127.0.0.1:9527

# 终端 3: 测试复制 (需要 xclip)
echo "Hello from X11" | xclip -selection clipboard

# 查看剪贴板内容
xclip -selection clipboard -o
```

## 生产环境部署

### 编译 Release 版本
```bash
cargo build --release
```

### 作为后台服务运行

#### macOS (使用 launchd)
创建 `~/Library/LaunchAgents/com.clipboard-sync.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.clipboard-sync</string>
    <key>ProgramArguments</key>
    <array>
        <string>/path/to/clipboard-sync</string>
        <string>server</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
```

加载服务:
```bash
launchctl load ~/Library/LaunchAgents/com.clipboard-sync.plist
```

#### Linux (使用 systemd)
创建 `/etc/systemd/system/clipboard-sync.service`:

```ini
[Unit]
Description=Clipboard Sync Service
After=network.target

[Service]
Type=simple
User=yourusername
ExecStart=/path/to/clipboard-sync server
Restart=always

[Install]
WantedBy=multi-user.target
```

启动服务:
```bash
sudo systemctl enable clipboard-sync
sudo systemctl start clipboard-sync
```

## 故障排除

### 无法连接到服务器
- 检查服务器是否正在运行
- 检查防火墙设置是否允许相应端口
- 确认使用正确的 IP 地址和端口

### 剪贴板未同步
- 确认剪贴板内容是文本格式（当前版本仅支持文本）
- 检查两端的日志输出
- 确认网络连接稳定

### Linux 上剪贴板不工作

**对于 Wayland：**
- 确保安装了 `wl-clipboard`：
  ```bash
  # Ubuntu/Debian
  sudo apt install wl-clipboard

  # Fedora
  sudo dnf install wl-clipboard

  # Arch Linux
  sudo pacman -S wl-clipboard
  ```
- 检查环境变量：`echo $WAYLAND_DISPLAY` 应该有输出（如 `wayland-0`）
- 程序启动时会显示 "Detected Wayland, using wl-clipboard backend"

**对于 X11：**
- 确保安装了必要的系统依赖（参见 README.md）
- 检查环境变量：`echo $DISPLAY` 应该有输出（如 `:0`）

**已知修复的问题：**
- ✅ Wayland 上剪贴板同步现在已支持
- ✅ 修复了只能接收一次数据的问题（之前是锁竞争导致的）
- ✅ 支持图片剪贴板同步（PNG 格式）

## 图片同步测试

### macOS 上测试图片复制

1. 在 Finder 中找一张图片，按 Cmd+C 复制
2. 或者在网页上右键图片，选择"复制图片"
3. 程序会显示：`Local clipboard changed, sending to server: image (1920x1080)`
4. 在另一台机器上打开任意支持粘贴图片的程序（如图片编辑器），按 Ctrl+V/Cmd+V 粘贴

### Linux 上测试图片复制

**使用 GNOME Screenshot:**
```bash
# 截图到剪贴板
gnome-screenshot -c

# 或使用其他截图工具如 Spectacle (KDE)
spectacle -r -c
```

**从文件复制图片（需要支持的文件管理器）:**
- 在文件管理器中右键图片 → 复制
- 或使用命令行：`cat image.png | wl-copy --type image/png`
