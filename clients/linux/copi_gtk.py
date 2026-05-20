#!/usr/bin/env python3
import base64
import json
import os
import secrets
import shutil
import signal
import socket
import subprocess
import sys
import threading
import time
import uuid
from dataclasses import asdict, dataclass, field
from pathlib import Path

import gi

gi.require_version("Gtk", "3.0")
from gi.repository import Gdk, GLib, Gtk

try:
    gi.require_version("AyatanaAppIndicator3", "0.1")
    from gi.repository import AyatanaAppIndicator3 as AppIndicator
except (ImportError, ValueError):
    try:
        gi.require_version("AppIndicator3", "0.1")
        from gi.repository import AppIndicator3 as AppIndicator
    except (ImportError, ValueError):
        AppIndicator = None


APP_ID = "com.cyole.copi"
APP_NAME = "Copi"
TRAY_ICON_NAME = f"{APP_ID}-symbolic"
CLIENT_DIR = Path(__file__).resolve().parent
REPO_ROOT = CLIENT_DIR.parents[1]
ICON_PATH = CLIENT_DIR / "assets" / "com.cyole.copi-symbolic.svg"
CONFIG_PATH = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "copi" / "linux-gui.json"
AUTOSTART_PATH = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "autostart" / f"{APP_ID}.desktop"
LOG_LIMIT = 500


def generate_sync_key(byte_count=24):
    return base64.b64encode(secrets.token_bytes(byte_count)).decode("ascii")


def generate_device_id():
    return f"linux-{uuid.uuid4()}".lower()


def default_device_name():
    return socket.gethostname() or "Linux"


@dataclass
class AppSettings:
    mode: str = "relay"
    device_name: str = field(default_factory=default_device_name)
    device_id: str = field(default_factory=generate_device_id)
    relay_url: str = "http://127.0.0.1:9527"
    access_token: str = ""
    sync_key: str = field(default_factory=generate_sync_key)
    launch_at_login: bool = False

    @classmethod
    def load(cls):
        if not CONFIG_PATH.exists():
            return cls()
        try:
            data = json.loads(CONFIG_PATH.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            return cls()

        settings = cls()
        for key in asdict(settings):
            if key in data:
                setattr(settings, key, data[key])
        if settings.mode not in ("relay", "lan"):
            settings.mode = "relay"
        if not settings.device_name:
            settings.device_name = default_device_name()
        if not settings.device_id:
            settings.device_id = generate_device_id()
        if not settings.sync_key:
            settings.sync_key = generate_sync_key()
        return settings

    def save(self):
        CONFIG_PATH.parent.mkdir(parents=True, exist_ok=True)
        CONFIG_PATH.write_text(json.dumps(asdict(self), ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        sync_autostart(self.launch_at_login)

    @property
    def mode_title(self):
        return "局域网" if self.mode == "lan" else "中转模式"

    @property
    def secret(self):
        return self.sync_key if self.mode == "lan" else self.access_token

    @property
    def is_runnable(self):
        return self.mode == "lan" or bool(self.relay_url.strip())


def resolve_cli_path():
    candidates = [
        os.environ.get("COPI_CLI"),
        str(CLIENT_DIR / "build" / "copi"),
        str(CLIENT_DIR / "copi"),
        str(REPO_ROOT / "copi"),
        shutil.which("copi"),
    ]
    for candidate in candidates:
        if candidate and Path(candidate).exists():
            return candidate
    return None


def sync_autostart(enabled):
    if not enabled:
        try:
            AUTOSTART_PATH.unlink()
        except FileNotFoundError:
            pass
        return

    AUTOSTART_PATH.parent.mkdir(parents=True, exist_ok=True)
    command = f"{desktop_exec_quote(sys.executable)} {desktop_exec_quote(str(Path(__file__).resolve()))}"
    AUTOSTART_PATH.write_text(
        "\n".join(
            [
                "[Desktop Entry]",
                "Type=Application",
                f"Name={APP_NAME}",
                f"Exec={command}",
                f"Icon={APP_ID}",
                "Terminal=false",
                "X-GNOME-Autostart-enabled=true",
                "Categories=Utility;",
                "",
            ]
        ),
        encoding="utf-8",
    )


def desktop_exec_quote(value):
    escaped = value.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


class ProcessController:
    def __init__(self, app):
        self.app = app
        self.process = None
        self.status = "stopped"
        self.last_error = ""

    @property
    def is_running(self):
        return self.process is not None and self.process.poll() is None

    @property
    def status_title(self):
        if self.status == "starting":
            return "启动中"
        if self.status == "running":
            return f"运行中 · {self.process.pid}" if self.process else "运行中"
        if self.status == "stopping":
            return "停止中"
        if self.status == "failed":
            return "启动失败"
        return "已停止"

    def start(self, settings):
        if self.is_running or self.status in ("starting", "stopping"):
            return
        if not settings.is_runnable:
            self.fail("请补全运行参数")
            return

        cli_path = resolve_cli_path()
        if not cli_path:
            self.fail("找不到 copi CLI，请先运行 clients/linux/run.sh 或设置 COPI_CLI")
            return

        args = [cli_path, "client"]
        if settings.mode == "lan":
            args.append("--lan")
        else:
            args.extend(["--relay", settings.relay_url.strip()])

        if settings.secret:
            args.extend(["--token", settings.secret])

        args.extend(
            [
                "--id",
                settings.device_id,
                "--name",
                settings.device_name,
                "--log-format",
                "json",
            ]
        )

        self.set_status("starting")
        try:
            self.process = subprocess.Popen(
                args,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1,
                start_new_session=True,
            )
        except OSError as error:
            self.process = None
            self.fail(str(error))
            return

        self.set_status("running")
        self.app.append_log("info", "gui", "copi client 已启动")
        threading.Thread(target=self.read_pipe, args=(self.process.stdout, "stdout"), daemon=True).start()
        threading.Thread(target=self.read_pipe, args=(self.process.stderr, "stderr"), daemon=True).start()
        threading.Thread(target=self.monitor, args=(self.process,), daemon=True).start()

    def stop(self):
        if not self.is_running:
            self.process = None
            self.set_status("stopped")
            return

        process = self.process
        self.set_status("stopping")
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except OSError:
            process.terminate()
        threading.Thread(target=self.wait_for_stop, args=(process,), daemon=True).start()

    def wait_for_stop(self, process):
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except OSError:
                process.kill()

    def read_pipe(self, pipe, source):
        if pipe is None:
            return
        for line in pipe:
            line = line.strip()
            if line:
                GLib.idle_add(self.app.consume_process_line, line, source)

    def monitor(self, process):
        code = process.wait()
        GLib.idle_add(self.handle_exit, process, code)

    def handle_exit(self, process, code):
        if self.process is not process:
            return False
        self.process = None
        if code == 0 or self.status == "stopping":
            self.set_status("stopped")
        else:
            self.fail(f"copi exited with status {code}")
        return False

    def fail(self, message):
        self.last_error = message
        self.app.append_log("error", "gui", message)
        self.set_status("failed")

    def set_status(self, status):
        self.status = status
        self.app.refresh_status()


class CopiLinuxApp:
    def __init__(self):
        self.settings = AppSettings.load()
        self.process = ProcessController(self)
        self.logs = []
        self.settings_window = None
        self.logs_window = None
        self.log_buffer = None
        self.widgets = {}

        self.install_css()
        self.build_indicator()
        sync_autostart(self.settings.launch_at_login)
        self.refresh_status()

    def install_css(self):
        css = b"""
        .copi-title { font-size: 20px; font-weight: 700; }
        .copi-muted { color: @theme_fg_color; opacity: 0.65; }
        .copi-section-title { font-weight: 700; }
        .copi-monospace { font-family: monospace; }
        .copi-header { padding: 18px; }
        .copi-content { padding: 18px; }
        """
        provider = Gtk.CssProvider()
        provider.load_from_data(css)
        screen = Gdk.Screen.get_default()
        if screen:
            Gtk.StyleContext.add_provider_for_screen(screen, provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)

    def build_indicator(self):
        self.menu = Gtk.Menu()

        self.status_item = Gtk.MenuItem(label="已停止")
        self.status_item.set_sensitive(False)
        self.menu.append(self.status_item)

        self.mode_item = Gtk.MenuItem(label=self.settings.mode_title)
        self.mode_item.set_sensitive(False)
        self.menu.append(self.mode_item)
        self.menu.append(Gtk.SeparatorMenuItem())

        self.toggle_item = Gtk.MenuItem(label="启动同步")
        self.toggle_item.connect("activate", self.on_toggle_sync)
        self.menu.append(self.toggle_item)

        logs_item = Gtk.MenuItem(label="日志...")
        logs_item.connect("activate", lambda *_: self.show_logs())
        self.menu.append(logs_item)

        settings_item = Gtk.MenuItem(label="设置...")
        settings_item.connect("activate", lambda *_: self.show_settings())
        self.menu.append(settings_item)
        self.menu.append(Gtk.SeparatorMenuItem())

        quit_item = Gtk.MenuItem(label="退出 Copi")
        quit_item.connect("activate", self.quit)
        self.menu.append(quit_item)
        self.menu.show_all()

        if AppIndicator:
            self.indicator = AppIndicator.Indicator.new(
                APP_ID,
                TRAY_ICON_NAME,
                AppIndicator.IndicatorCategory.APPLICATION_STATUS,
            )
            if hasattr(self.indicator, "set_icon_theme_path"):
                self.indicator.set_icon_theme_path(str(ICON_PATH.parent))
            if hasattr(self.indicator, "set_icon_full"):
                self.indicator.set_icon_full(TRAY_ICON_NAME, APP_NAME)
            self.indicator.set_status(AppIndicator.IndicatorStatus.ACTIVE)
            self.indicator.set_menu(self.menu)
            self.status_icon = None
        else:
            self.indicator = None
            self.status_icon = Gtk.StatusIcon.new_from_file(str(ICON_PATH))
            self.status_icon.set_tooltip_text(APP_NAME)
            self.status_icon.connect("activate", lambda *_: self.show_settings())
            self.status_icon.connect("popup-menu", self.on_status_icon_popup)

    def on_status_icon_popup(self, icon, button, activate_time):
        self.menu.popup(None, None, Gtk.StatusIcon.position_menu, icon, button, activate_time)

    def refresh_status(self):
        GLib.idle_add(self._refresh_status)

    def _refresh_status(self):
        title = self.process.status_title
        self.status_item.set_label(title)
        self.mode_item.set_label(self.settings.mode_title)
        self.toggle_item.set_label("停止同步" if self.process.is_running else "启动同步")

        tooltip = f"{APP_NAME} · {title} · {self.settings.mode_title}"
        if self.indicator and hasattr(self.indicator, "set_title"):
            self.indicator.set_title(tooltip)
        if self.status_icon:
            self.status_icon.set_tooltip_text(tooltip)

        if self.settings_window:
            self.update_settings_window_state()
        return False

    def show_settings(self):
        if self.settings_window:
            self.settings_window.present()
            return

        window = Gtk.Window(title="设置")
        window.set_default_size(760, 560)
        window.set_position(Gtk.WindowPosition.CENTER)
        window.connect("delete-event", self.hide_window)
        self.settings_window = window

        root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        window.add(root)

        root.pack_start(self.build_settings_header(), False, False, 0)
        root.pack_start(Gtk.Separator(), False, False, 0)

        scroller = Gtk.ScrolledWindow()
        scroller.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        root.pack_start(scroller, True, True, 0)

        content = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=16)
        content.get_style_context().add_class("copi-content")
        self.widgets["settings_content"] = content
        scroller.add(content)

        content.pack_start(self.build_sync_section(), False, False, 0)
        content.pack_start(self.build_device_section(), False, False, 0)
        content.pack_start(self.build_behavior_section(), False, False, 0)

        self.refresh_mode_visibility()
        self.update_settings_window_state()
        window.show_all()
        self.refresh_mode_visibility()

    def build_settings_header(self):
        header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=14)
        header.get_style_context().add_class("copi-header")

        image = Gtk.Image.new_from_file(str(ICON_PATH))
        image.set_pixel_size(40)
        header.pack_start(image, False, False, 0)

        title_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        title = Gtk.Label(label="Copi")
        title.set_xalign(0)
        title.get_style_context().add_class("copi-title")
        title_box.pack_start(title, False, False, 0)

        subtitle = Gtk.Label(label="")
        subtitle.set_xalign(0)
        subtitle.get_style_context().add_class("copi-muted")
        self.widgets["status_label"] = subtitle
        title_box.pack_start(subtitle, False, False, 0)
        header.pack_start(title_box, True, True, 0)

        toggle = Gtk.Button(label="启动")
        toggle.connect("clicked", self.on_toggle_sync)
        self.widgets["header_toggle"] = toggle
        header.pack_start(toggle, False, False, 0)
        return header

    def build_sync_section(self):
        section = self.section_box("同步")

        mode_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        mode_box.pack_start(Gtk.Label(label="模式"), False, False, 0)
        relay_radio = Gtk.RadioButton.new_with_label_from_widget(None, "中转模式")
        lan_radio = Gtk.RadioButton.new_with_label_from_widget(relay_radio, "局域网")
        relay_radio.set_active(self.settings.mode == "relay")
        lan_radio.set_active(self.settings.mode == "lan")
        relay_radio.connect("toggled", self.on_mode_changed, "relay")
        lan_radio.connect("toggled", self.on_mode_changed, "lan")
        self.widgets["relay_radio"] = relay_radio
        self.widgets["lan_radio"] = lan_radio
        mode_box.pack_start(relay_radio, False, False, 0)
        mode_box.pack_start(lan_radio, False, False, 0)
        section.pack_start(mode_box, False, False, 0)

        relay_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        self.widgets["relay_box"] = relay_box
        relay_url = Gtk.Entry()
        relay_url.set_text(self.settings.relay_url)
        relay_url.set_placeholder_text("http://127.0.0.1:9527")
        relay_url.connect("changed", self.on_entry_changed, "relay_url")
        self.widgets["relay_url"] = relay_url
        relay_box.pack_start(self.row("地址", relay_url), False, False, 0)

        access_token = Gtk.Entry()
        access_token.set_text(self.settings.access_token)
        access_token.set_visibility(False)
        access_token.connect("changed", self.on_entry_changed, "access_token")
        self.widgets["access_token"] = access_token
        relay_box.pack_start(self.row("访问令牌", access_token), False, False, 0)
        section.pack_start(relay_box, False, False, 0)

        lan_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        self.widgets["lan_box"] = lan_box
        sync_key = Gtk.Entry()
        sync_key.set_text(self.settings.sync_key)
        sync_key.set_visibility(False)
        sync_key.connect("changed", self.on_entry_changed, "sync_key")
        self.widgets["sync_key"] = sync_key
        key_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        key_row.pack_start(sync_key, True, True, 0)
        key_button = Gtk.Button(label="生成")
        key_button.connect("clicked", self.generate_sync_key)
        key_row.pack_start(key_button, False, False, 0)
        lan_box.pack_start(self.row("同步密钥", key_row), False, False, 0)
        section.pack_start(lan_box, False, False, 0)

        return section

    def build_device_section(self):
        section = self.section_box("设备")

        device_name = Gtk.Entry()
        device_name.set_text(self.settings.device_name)
        device_name.connect("changed", self.on_entry_changed, "device_name")
        self.widgets["device_name"] = device_name
        section.pack_start(self.row("名称", device_name), False, False, 0)

        device_id = Gtk.Entry()
        device_id.set_text(self.settings.device_id)
        device_id.get_style_context().add_class("copi-monospace")
        device_id.connect("changed", self.on_entry_changed, "device_id")
        self.widgets["device_id"] = device_id
        id_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        id_row.pack_start(device_id, True, True, 0)
        generate = Gtk.Button(label="生成")
        generate.connect("clicked", self.generate_device_id)
        id_row.pack_start(generate, False, False, 0)
        section.pack_start(self.row("ID", id_row), False, False, 0)

        return section

    def build_behavior_section(self):
        section = self.section_box("行为")
        launch = Gtk.CheckButton(label="登录后自动启动 Copi")
        launch.set_active(self.settings.launch_at_login)
        launch.connect("toggled", self.on_launch_at_login_changed)
        self.widgets["launch_at_login"] = launch
        section.pack_start(launch, False, False, 0)
        return section

    def section_box(self, title):
        inner = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        inner.set_border_width(14)

        label = Gtk.Label(label=title)
        label.set_xalign(0)
        label.get_style_context().add_class("copi-section-title")
        inner.pack_start(label, False, False, 0)
        return inner

    def row(self, title, widget):
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        label = Gtk.Label(label=title)
        label.set_size_request(82, -1)
        label.set_xalign(1)
        label.get_style_context().add_class("copi-muted")
        box.pack_start(label, False, False, 0)
        box.pack_start(widget, True, True, 0)
        return box

    def update_settings_window_state(self):
        status_text = f"{self.process.status_title} · {self.settings.mode_title}"
        self.widgets["status_label"].set_text(status_text)
        self.widgets["header_toggle"].set_label("停止" if self.process.is_running else "启动")
        self.widgets["header_toggle"].set_sensitive(self.process.is_running or self.settings.is_runnable)
        content = self.widgets.get("settings_content")
        if content:
            content.set_sensitive(not self.process.is_running)

    def refresh_mode_visibility(self):
        relay_visible = self.settings.mode == "relay"
        self.widgets["relay_box"].set_visible(relay_visible)
        self.widgets["lan_box"].set_visible(not relay_visible)
        self.refresh_status()

    def show_logs(self):
        if self.logs_window:
            self.logs_window.present()
            return

        window = Gtk.Window(title="日志")
        window.set_default_size(760, 460)
        window.set_position(Gtk.WindowPosition.CENTER)
        window.connect("delete-event", self.hide_window)
        self.logs_window = window

        root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        window.add(root)

        toolbar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        toolbar.set_border_width(10)
        clear = Gtk.Button(label="清空")
        clear.connect("clicked", self.clear_logs)
        toolbar.pack_end(clear, False, False, 0)
        root.pack_start(toolbar, False, False, 0)

        scroller = Gtk.ScrolledWindow()
        scroller.set_policy(Gtk.PolicyType.AUTOMATIC, Gtk.PolicyType.AUTOMATIC)
        root.pack_start(scroller, True, True, 0)

        text = Gtk.TextView()
        text.set_editable(False)
        text.set_cursor_visible(False)
        text.set_monospace(True)
        scroller.add(text)
        self.log_buffer = text.get_buffer()
        self.refresh_log_buffer()
        window.show_all()

    def append_log(self, level, event_type, message):
        payload = {
            "time": time.strftime("%H:%M:%S"),
            "level": level,
            "type": event_type,
            "message": message,
        }
        self.logs.append(payload)
        if len(self.logs) > LOG_LIMIT:
            self.logs = self.logs[-LOG_LIMIT:]
        self.refresh_log_buffer()

    def consume_process_line(self, line, source):
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            payload = {
                "time": time.strftime("%H:%M:%S"),
                "level": "error" if source == "stderr" else "info",
                "type": source,
                "message": line,
            }
        if "time" not in payload:
            payload["time"] = time.strftime("%H:%M:%S")
        self.logs.append(payload)
        if len(self.logs) > LOG_LIMIT:
            self.logs = self.logs[-LOG_LIMIT:]
        self.refresh_log_buffer()
        return False

    def refresh_log_buffer(self):
        if not self.log_buffer:
            return
        lines = []
        for item in self.logs:
            timestamp = str(item.get("time", ""))[-18:]
            level = item.get("level", "info")
            event_type = item.get("type", "event")
            message = item.get("message", "")
            lines.append(f"[{timestamp}] {level} {event_type}: {message}")
        self.log_buffer.set_text("\n".join(lines))

    def clear_logs(self, *_):
        self.logs = []
        self.refresh_log_buffer()

    def hide_window(self, window, _event):
        window.hide()
        return True

    def on_toggle_sync(self, *_):
        self.save_from_widgets()
        if self.process.is_running:
            self.process.stop()
        else:
            self.process.start(self.settings)

    def on_mode_changed(self, button, mode):
        if not button.get_active():
            return
        self.settings.mode = mode
        self.settings.save()
        self.refresh_mode_visibility()

    def on_entry_changed(self, entry, key):
        setattr(self.settings, key, entry.get_text())
        self.settings.save()
        self.refresh_status()

    def on_launch_at_login_changed(self, button):
        self.settings.launch_at_login = button.get_active()
        self.settings.save()

    def generate_sync_key(self, *_):
        self.widgets["sync_key"].set_text(generate_sync_key())

    def generate_device_id(self, *_):
        self.widgets["device_id"].set_text(generate_device_id())

    def save_from_widgets(self):
        for key in ("relay_url", "access_token", "sync_key", "device_name", "device_id"):
            widget = self.widgets.get(key)
            if widget:
                setattr(self.settings, key, widget.get_text())
        self.settings.save()

    def quit(self, *_):
        if self.process.is_running:
            self.process.stop()
        Gtk.main_quit()


def main():
    app = CopiLinuxApp()
    if not AppIndicator:
        app.show_settings()
    Gtk.main()


if __name__ == "__main__":
    main()
