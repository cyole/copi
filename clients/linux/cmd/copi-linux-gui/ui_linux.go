//go:build linux

package main

import (
	"fmt"
	"html"
	"os"
	"strings"
	"time"

	"github.com/gotk3/gotk3/gdk"
	"github.com/gotk3/gotk3/glib"
	"github.com/gotk3/gotk3/gtk"
)

const logLimit = 500

type linuxApp struct {
	settings appSettings
	process  *processController
	logs     []logEvent
	iconPath string

	statusIcon *trayIcon
	menu       *gtk.Menu
	statusItem *gtk.MenuItem
	modeItem   *gtk.MenuItem
	toggleItem *gtk.MenuItem

	windowStatusItem *gtk.MenuItem
	windowModeItem   *gtk.MenuItem
	windowToggleItem *gtk.MenuItem

	settingsWindow *gtk.Window
	logsWindow     *gtk.Window

	settingsStatus *gtk.Label
	headerToggle   *gtk.Button
	syncSection    *gtk.Box
	deviceSection  *gtk.Box
	relayBox       *gtk.Box
	lanBox         *gtk.Box

	relayRadio *gtk.RadioButton
	lanRadio   *gtk.RadioButton

	relayEntry  *gtk.Entry
	tokenEntry  *gtk.Entry
	syncKey     *gtk.Entry
	deviceName  *gtk.Entry
	deviceID    *gtk.Entry
	launchLogin *gtk.CheckButton

	logBuffer *gtk.TextBuffer
}

func newLinuxApp() (*linuxApp, error) {
	settings, err := loadSettings()
	if err != nil {
		settings = defaultSettings()
	}

	app := &linuxApp{
		settings: settings,
		iconPath: resolveIconPath(),
	}
	app.process = newProcessController(app)
	app.installCSS()
	if err := app.buildTray(); err != nil {
		return nil, err
	}
	if err := syncAutostart(app.settings.LaunchAtLogin); err != nil {
		app.appendLog(logEvent{
			ReceivedAt: time.Now(),
			Source:     "gui",
			Level:      "warn",
			Type:       "autostart",
			Message:    err.Error(),
		})
	}
	app.refreshStatus()
	return app, nil
}

func (a *linuxApp) run() {
	if a.statusIcon == nil {
		a.showSettings()
		return
	}

	glib.TimeoutAdd(1000, func() bool {
		if a.statusIcon != nil && !a.statusIcon.IsEmbedded() {
			a.showSettings()
		}
		return false
	})
}

func runningWaylandSession() bool {
	return strings.EqualFold(os.Getenv("XDG_SESSION_TYPE"), "wayland") || os.Getenv("WAYLAND_DISPLAY") != ""
}

func (a *linuxApp) installCSS() {
	provider, err := gtk.CssProviderNew()
	if err != nil {
		return
	}
	_ = provider.LoadFromData(`
		.copi-content { padding: 18px; }
		.copi-title { font-size: 20px; font-weight: 700; }
		.copi-muted { opacity: 0.68; }
		.copi-section-title { font-weight: 700; }
		.copi-error { color: #c01c28; }
	`)
	screen, err := gdk.ScreenGetDefault()
	if err == nil {
		gtk.AddProviderForScreen(screen, provider, gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
	}
}

func (a *linuxApp) buildTray() error {
	menu, err := gtk.MenuNew()
	if err != nil {
		return err
	}
	a.menu = menu

	a.statusItem = must(gtk.MenuItemNewWithLabel("已停止"))
	a.statusItem.SetSensitive(false)
	menu.Append(a.statusItem)

	a.modeItem = must(gtk.MenuItemNewWithLabel(a.settings.modeTitle()))
	a.modeItem.SetSensitive(false)
	menu.Append(a.modeItem)
	menu.Append(must(gtk.SeparatorMenuItemNew()))

	a.toggleItem = must(gtk.MenuItemNewWithLabel("启动同步"))
	a.toggleItem.Connect("activate", func() {
		a.toggleSync()
	})
	menu.Append(a.toggleItem)

	logsItem := must(gtk.MenuItemNewWithLabel("日志..."))
	logsItem.Connect("activate", func() {
		a.showLogs()
	})
	menu.Append(logsItem)

	settingsItem := must(gtk.MenuItemNewWithLabel("设置..."))
	settingsItem.Connect("activate", func() {
		a.showSettings()
	})
	menu.Append(settingsItem)
	menu.Append(must(gtk.SeparatorMenuItemNew()))

	quitItem := must(gtk.MenuItemNewWithLabel("退出 Copi"))
	quitItem.Connect("activate", func() {
		a.quit()
	})
	menu.Append(quitItem)
	menu.ShowAll()

	if a.iconPath == "" {
		a.statusIcon = must(newTrayIconFromIconName("network-server-symbolic"))
	} else {
		a.statusIcon = must(newTrayIconFromFile(a.iconPath))
	}
	a.statusIcon.SetTitle(appName)
	a.statusIcon.ConnectActivate(func() {
		a.showSettings()
	})
	a.statusIcon.ConnectPopupMenu(func(button uint, activateTime uint32) {
		a.statusIcon.PopupMenu(a.menu, button, activateTime)
	})
	a.statusIcon.SetMenu(a.menu)
	a.statusIcon.SetVisible(true)
	return nil
}

func (a *linuxApp) showSettings() {
	if a.settingsWindow != nil {
		a.settingsWindow.Present()
		return
	}

	window := must(gtk.WindowNew(gtk.WINDOW_TOPLEVEL))
	window.SetTitle("设置")
	window.SetDefaultSize(760, 560)
	window.SetPosition(gtk.WIN_POS_CENTER)
	window.Connect("delete-event", func() bool {
		if a.statusIcon == nil {
			a.quit()
			return true
		}
		window.Hide()
		return true
	})
	a.settingsWindow = window

	root := must(gtk.BoxNew(gtk.ORIENTATION_VERTICAL, 0))
	window.Add(root)

	root.PackStart(a.buildSettingsMenuBar(), false, false, 0)
	root.PackStart(must(gtk.SeparatorNew(gtk.ORIENTATION_HORIZONTAL)), false, false, 0)
	root.PackStart(a.buildSettingsHeader(), false, false, 0)
	root.PackStart(must(gtk.SeparatorNew(gtk.ORIENTATION_HORIZONTAL)), false, false, 0)

	scroller := must(gtk.ScrolledWindowNew(nil, nil))
	scroller.SetPolicy(gtk.POLICY_NEVER, gtk.POLICY_AUTOMATIC)
	root.PackStart(scroller, true, true, 0)

	content := must(gtk.BoxNew(gtk.ORIENTATION_VERTICAL, 16))
	content.SetMarginTop(18)
	content.SetMarginBottom(18)
	content.SetMarginStart(18)
	content.SetMarginEnd(18)
	scroller.Add(content)

	a.syncSection = a.buildSyncSection()
	a.deviceSection = a.buildDeviceSection()
	content.PackStart(a.syncSection, false, false, 0)
	content.PackStart(a.deviceSection, false, false, 0)
	content.PackStart(a.buildBehaviorSection(), false, false, 0)

	a.refreshModeVisibility()
	a.updateSettingsState()
	window.ShowAll()
	a.refreshModeVisibility()
	window.Present()
}

func (a *linuxApp) buildSettingsMenuBar() *gtk.MenuBar {
	menuBar := must(gtk.MenuBarNew())

	appItem := must(gtk.MenuItemNewWithLabel("Copi"))
	appMenu := must(gtk.MenuNew())

	a.windowStatusItem = must(gtk.MenuItemNewWithLabel(a.process.statusTitle()))
	a.windowStatusItem.SetSensitive(false)
	appMenu.Append(a.windowStatusItem)

	a.windowModeItem = must(gtk.MenuItemNewWithLabel(a.settings.modeTitle()))
	a.windowModeItem.SetSensitive(false)
	appMenu.Append(a.windowModeItem)
	appMenu.Append(must(gtk.SeparatorMenuItemNew()))

	a.windowToggleItem = must(gtk.MenuItemNewWithLabel("启动同步"))
	a.windowToggleItem.Connect("activate", func() {
		a.toggleSync()
	})
	appMenu.Append(a.windowToggleItem)

	logsItem := must(gtk.MenuItemNewWithLabel("日志..."))
	logsItem.Connect("activate", func() {
		a.showLogs()
	})
	appMenu.Append(logsItem)
	appMenu.Append(must(gtk.SeparatorMenuItemNew()))

	quitItem := must(gtk.MenuItemNewWithLabel("退出 Copi"))
	quitItem.Connect("activate", func() {
		a.quit()
	})
	appMenu.Append(quitItem)

	appItem.SetSubmenu(appMenu)
	menuBar.Append(appItem)
	return menuBar
}

func (a *linuxApp) buildSettingsHeader() *gtk.Box {
	header := must(gtk.BoxNew(gtk.ORIENTATION_HORIZONTAL, 14))
	header.SetMarginTop(18)
	header.SetMarginBottom(18)
	header.SetMarginStart(22)
	header.SetMarginEnd(22)

	image := must(gtk.ImageNewFromIconName("network-server-symbolic", gtk.ICON_SIZE_DIALOG))
	if a.iconPath != "" {
		if pixbuf, err := gdk.PixbufNewFromFileAtScale(a.iconPath, 40, 40, true); err == nil {
			if fileImage, err := gtk.ImageNewFromPixbuf(pixbuf); err == nil {
				image = fileImage
			}
		}
	}
	image.SetPixelSize(40)
	header.PackStart(image, false, false, 0)

	titleBox := must(gtk.BoxNew(gtk.ORIENTATION_VERTICAL, 4))
	title := must(gtk.LabelNew(""))
	title.SetMarkup("<span size='large' weight='bold'>Copi</span>")
	title.SetXAlign(0)
	titleBox.PackStart(title, false, false, 0)

	a.settingsStatus = must(gtk.LabelNew(""))
	a.settingsStatus.SetXAlign(0)
	titleBox.PackStart(a.settingsStatus, false, false, 0)
	header.PackStart(titleBox, true, true, 0)

	logsButton := must(gtk.ButtonNewWithLabel("日志"))
	logsButton.Connect("clicked", func() {
		a.showLogs()
	})
	header.PackStart(logsButton, false, false, 0)

	a.headerToggle = must(gtk.ButtonNewWithLabel("启动"))
	a.headerToggle.Connect("clicked", func() {
		a.toggleSync()
	})
	header.PackStart(a.headerToggle, false, false, 0)
	return header
}

func (a *linuxApp) buildSyncSection() *gtk.Box {
	section := sectionBox("同步")

	modeRow := must(gtk.BoxNew(gtk.ORIENTATION_HORIZONTAL, 10))
	modeLabel := mutedLabel("模式")
	modeLabel.SetSizeRequest(92, -1)
	modeRow.PackStart(modeLabel, false, false, 0)

	a.relayRadio = must(gtk.RadioButtonNewWithLabel(nil, "中转模式"))
	a.lanRadio = must(gtk.RadioButtonNewWithLabelFromWidget(a.relayRadio, "局域网"))
	a.relayRadio.SetActive(a.settings.Mode == modeRelay)
	a.lanRadio.SetActive(a.settings.Mode == modeLAN)
	a.relayRadio.Connect("toggled", func() {
		if a.relayRadio.GetActive() {
			a.settings.Mode = modeRelay
			a.saveSettings()
			a.refreshModeVisibility()
		}
	})
	a.lanRadio.Connect("toggled", func() {
		if a.lanRadio.GetActive() {
			a.settings.Mode = modeLAN
			a.saveSettings()
			a.refreshModeVisibility()
		}
	})
	modeRow.PackStart(a.relayRadio, false, false, 0)
	modeRow.PackStart(a.lanRadio, false, false, 0)
	section.PackStart(modeRow, false, false, 0)

	a.relayBox = must(gtk.BoxNew(gtk.ORIENTATION_VERTICAL, 10))
	a.relayEntry = must(gtk.EntryNew())
	a.relayEntry.SetText(a.settings.RelayURL)
	a.relayEntry.SetPlaceholderText(defaultRelayURL)
	a.relayEntry.Connect("changed", func() {
		a.settings.RelayURL = entryText(a.relayEntry)
		a.saveSettings()
	})
	a.relayBox.PackStart(row("地址", a.relayEntry), false, false, 0)

	a.tokenEntry = must(gtk.EntryNew())
	a.tokenEntry.SetText(a.settings.AccessToken)
	a.tokenEntry.SetVisibility(false)
	a.tokenEntry.Connect("changed", func() {
		a.settings.AccessToken = entryText(a.tokenEntry)
		a.saveSettings()
	})
	a.relayBox.PackStart(row("访问令牌", a.tokenEntry), false, false, 0)
	section.PackStart(a.relayBox, false, false, 0)

	a.lanBox = must(gtk.BoxNew(gtk.ORIENTATION_VERTICAL, 10))
	keyRow := must(gtk.BoxNew(gtk.ORIENTATION_HORIZONTAL, 8))
	a.syncKey = must(gtk.EntryNew())
	a.syncKey.SetText(a.settings.SyncKey)
	a.syncKey.SetVisibility(false)
	a.syncKey.Connect("changed", func() {
		a.settings.SyncKey = entryText(a.syncKey)
		a.saveSettings()
	})
	keyRow.PackStart(a.syncKey, true, true, 0)

	keyButton := must(gtk.ButtonNewWithLabel("生成"))
	keyButton.Connect("clicked", func() {
		a.syncKey.SetText(generateSyncKey())
	})
	keyRow.PackStart(keyButton, false, false, 0)
	a.lanBox.PackStart(row("同步密钥", keyRow), false, false, 0)
	section.PackStart(a.lanBox, false, false, 0)

	return section
}

func (a *linuxApp) buildDeviceSection() *gtk.Box {
	section := sectionBox("设备")

	a.deviceName = must(gtk.EntryNew())
	a.deviceName.SetText(a.settings.DeviceName)
	a.deviceName.Connect("changed", func() {
		a.settings.DeviceName = entryText(a.deviceName)
		a.saveSettings()
	})
	section.PackStart(row("名称", a.deviceName), false, false, 0)

	idRow := must(gtk.BoxNew(gtk.ORIENTATION_HORIZONTAL, 8))
	a.deviceID = must(gtk.EntryNew())
	a.deviceID.SetText(a.settings.DeviceID)
	a.deviceID.Connect("changed", func() {
		a.settings.DeviceID = entryText(a.deviceID)
		a.saveSettings()
	})
	idRow.PackStart(a.deviceID, true, true, 0)

	generate := must(gtk.ButtonNewWithLabel("生成"))
	generate.Connect("clicked", func() {
		a.deviceID.SetText(generateDeviceID())
	})
	idRow.PackStart(generate, false, false, 0)
	section.PackStart(row("ID", idRow), false, false, 0)
	return section
}

func (a *linuxApp) buildBehaviorSection() *gtk.Box {
	section := sectionBox("行为")
	a.launchLogin = must(gtk.CheckButtonNewWithLabel("登录后自动启动 Copi"))
	a.launchLogin.SetActive(a.settings.LaunchAtLogin)
	a.launchLogin.Connect("toggled", func() {
		a.settings.LaunchAtLogin = a.launchLogin.GetActive()
		a.saveSettings()
	})
	section.PackStart(row("启动", a.launchLogin), false, false, 0)
	return section
}

func (a *linuxApp) showLogs() {
	if a.logsWindow != nil {
		a.logsWindow.Present()
		return
	}

	window := must(gtk.WindowNew(gtk.WINDOW_TOPLEVEL))
	window.SetTitle("日志")
	window.SetDefaultSize(760, 460)
	window.SetPosition(gtk.WIN_POS_CENTER)
	window.Connect("delete-event", func() bool {
		window.Hide()
		return true
	})
	a.logsWindow = window

	root := must(gtk.BoxNew(gtk.ORIENTATION_VERTICAL, 0))
	window.Add(root)

	toolbar := must(gtk.BoxNew(gtk.ORIENTATION_HORIZONTAL, 8))
	toolbar.SetMarginTop(10)
	toolbar.SetMarginBottom(10)
	toolbar.SetMarginStart(10)
	toolbar.SetMarginEnd(10)
	clearButton := must(gtk.ButtonNewWithLabel("清空"))
	clearButton.Connect("clicked", func() {
		a.logs = nil
		a.refreshLogBuffer()
	})
	toolbar.PackEnd(clearButton, false, false, 0)
	root.PackStart(toolbar, false, false, 0)

	scroller := must(gtk.ScrolledWindowNew(nil, nil))
	scroller.SetPolicy(gtk.POLICY_AUTOMATIC, gtk.POLICY_AUTOMATIC)
	root.PackStart(scroller, true, true, 0)

	text := must(gtk.TextViewNew())
	text.SetEditable(false)
	text.SetCursorVisible(false)
	text.SetMonospace(true)
	buffer, _ := text.GetBuffer()
	a.logBuffer = buffer
	scroller.Add(text)

	a.refreshLogBuffer()
	window.ShowAll()
	window.Present()
}

func (a *linuxApp) toggleSync() {
	a.saveFromWidgets()
	if a.process.isRunning() {
		a.process.stop()
		return
	}
	a.process.start(a.settings)
}

func (a *linuxApp) quit() {
	if a.process.isRunning() {
		a.process.stop()
	}
	gtk.MainQuit()
}

func (a *linuxApp) saveFromWidgets() {
	if a.relayEntry != nil {
		a.settings.RelayURL = entryText(a.relayEntry)
	}
	if a.tokenEntry != nil {
		a.settings.AccessToken = entryText(a.tokenEntry)
	}
	if a.syncKey != nil {
		a.settings.SyncKey = entryText(a.syncKey)
	}
	if a.deviceName != nil {
		a.settings.DeviceName = entryText(a.deviceName)
	}
	if a.deviceID != nil {
		a.settings.DeviceID = entryText(a.deviceID)
	}
	if a.launchLogin != nil {
		a.settings.LaunchAtLogin = a.launchLogin.GetActive()
	}
	a.settings.applyDefaults()
	a.saveSettings()
}

func (a *linuxApp) saveSettings() {
	a.settings.applyDefaults()
	if err := a.settings.save(); err != nil {
		a.appendLog(logEvent{
			ReceivedAt: time.Now(),
			Source:     "gui",
			Level:      "error",
			Type:       "settings_save_failed",
			Message:    err.Error(),
		})
	}
	a.refreshStatus()
}

func (a *linuxApp) refreshStatus() {
	if a.statusItem == nil {
		return
	}

	status := a.process.statusTitle()
	a.statusItem.SetLabel(status)
	a.modeItem.SetLabel(a.settings.modeTitle())
	if a.windowStatusItem != nil {
		a.windowStatusItem.SetLabel(status)
	}
	if a.windowModeItem != nil {
		a.windowModeItem.SetLabel(a.settings.modeTitle())
	}
	if a.process.isRunning() {
		a.toggleItem.SetLabel("停止同步")
		if a.windowToggleItem != nil {
			a.windowToggleItem.SetLabel("停止同步")
		}
	} else {
		a.toggleItem.SetLabel("启动同步")
		if a.windowToggleItem != nil {
			a.windowToggleItem.SetLabel("启动同步")
		}
	}

	tooltip := fmt.Sprintf("%s · %s · %s", appName, status, a.settings.modeTitle())
	if a.statusIcon != nil {
		a.statusIcon.SetTooltipText(tooltip)
	}
	a.updateSettingsState()
}

func (a *linuxApp) updateSettingsState() {
	if a.settingsStatus != nil {
		a.settingsStatus.SetMarkup("<span alpha='70%'>" + html.EscapeString(a.process.statusTitle()+" · "+a.settings.modeTitle()) + "</span>")
	}
	if a.headerToggle != nil {
		if a.process.isRunning() {
			a.headerToggle.SetLabel("停止")
			a.headerToggle.SetSensitive(true)
		} else {
			a.headerToggle.SetLabel("启动")
			a.headerToggle.SetSensitive(a.process.canStart() && a.settings.runnable())
		}
	}
	if a.windowToggleItem != nil {
		if a.process.isRunning() {
			a.windowToggleItem.SetLabel("停止同步")
			a.windowToggleItem.SetSensitive(true)
		} else {
			a.windowToggleItem.SetLabel("启动同步")
			a.windowToggleItem.SetSensitive(a.process.canStart() && a.settings.runnable())
		}
	}
	if a.syncSection != nil {
		a.syncSection.SetSensitive(!a.process.isRunning())
	}
	if a.deviceSection != nil {
		a.deviceSection.SetSensitive(!a.process.isRunning())
	}
}

func (a *linuxApp) refreshModeVisibility() {
	if a.relayBox == nil || a.lanBox == nil {
		return
	}
	relayVisible := a.settings.Mode == modeRelay
	a.relayBox.SetVisible(relayVisible)
	a.lanBox.SetVisible(!relayVisible)
	a.refreshStatus()
}

func (a *linuxApp) appendLog(event logEvent) {
	if event.ReceivedAt.IsZero() {
		event.ReceivedAt = time.Now()
	}
	a.logs = append(a.logs, event)
	if len(a.logs) > logLimit {
		a.logs = a.logs[len(a.logs)-logLimit:]
	}
	a.refreshLogBuffer()
}

func (a *linuxApp) consumeProcessLine(line, source string) {
	a.appendLog(parseLogLine(line, source))
}

func (a *linuxApp) refreshLogBuffer() {
	if a.logBuffer == nil {
		return
	}
	lines := make([]string, 0, len(a.logs))
	for _, event := range a.logs {
		level := event.Level
		if level == "" {
			level = "info"
		}
		eventType := event.Type
		if eventType == "" {
			eventType = "event"
		}
		lines = append(lines, fmt.Sprintf("[%s] %s %s: %s",
			event.ReceivedAt.Format("15:04:05"),
			level,
			eventType,
			event.Message,
		))
	}
	a.logBuffer.SetText(strings.Join(lines, "\n"))
}

func sectionBox(title string) *gtk.Box {
	section := must(gtk.BoxNew(gtk.ORIENTATION_VERTICAL, 12))
	section.SetMarginTop(14)
	section.SetMarginBottom(14)
	section.SetMarginStart(14)
	section.SetMarginEnd(14)

	label := must(gtk.LabelNew(""))
	label.SetMarkup("<b>" + html.EscapeString(title) + "</b>")
	label.SetXAlign(0)
	section.PackStart(label, false, false, 0)
	return section
}

func row(title string, widget gtk.IWidget) *gtk.Box {
	box := must(gtk.BoxNew(gtk.ORIENTATION_HORIZONTAL, 12))
	label := mutedLabel(title)
	label.SetSizeRequest(92, -1)
	box.PackStart(label, false, false, 0)
	box.PackStart(widget, true, true, 0)
	return box
}

func mutedLabel(text string) *gtk.Label {
	label := must(gtk.LabelNew(""))
	label.SetMarkup("<span alpha='70%'>" + html.EscapeString(text) + "</span>")
	label.SetXAlign(1)
	return label
}

func entryText(entry *gtk.Entry) string {
	text, err := entry.GetText()
	if err != nil {
		return ""
	}
	return text
}

func must[T any](value T, err error) T {
	if err != nil {
		panic(err)
	}
	return value
}
