import AppKit
import SwiftUI

@main
struct CopiApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var settings = SettingsStore()
    @StateObject private var processController = CopiProcessController()

    var body: some Scene {
        MenuBarExtra {
            MenuBarContentView()
                .environmentObject(settings)
                .environmentObject(processController)
        } label: {
            Label("Copi", systemImage: processController.isRunning ? "arrow.triangle.2.circlepath.circle.fill" : "arrow.triangle.2.circlepath.circle")
        }
        .menuBarExtraStyle(.menu)

        Window("设置", id: "settings") {
            SettingsView()
                .environmentObject(settings)
                .environmentObject(processController)
                .frame(width: 560, height: 420)
                .padding()
        }

        Window("日志", id: "logs") {
            LogsView()
                .environmentObject(processController)
                .frame(minWidth: 680, minHeight: 420)
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
    }
}

private struct MenuBarContentView: View {
    @Environment(\.openWindow) private var openWindow
    @EnvironmentObject private var settings: SettingsStore
    @EnvironmentObject private var processController: CopiProcessController

    var body: some View {
        Text(processController.status.title)
            .font(.headline)

        Text(settings.mode.title)
            .foregroundStyle(.secondary)

        Divider()

        Button {
            toggleProcess()
        } label: {
            Label(processController.isRunning ? "停止同步" : "启动同步", systemImage: processController.isRunning ? "stop.fill" : "play.fill")
        }
        .disabled(!canToggle)

        Button {
            openAndFocusWindow(id: "logs", title: "日志")
        } label: {
            Label("日志...", systemImage: "doc.text.magnifyingglass")
        }

        Button {
            openAndFocusWindow(id: "settings", title: "设置")
        } label: {
            Label("设置...", systemImage: "gearshape")
        }

        Divider()

        Button {
            processController.stop()
            NSApp.terminate(nil)
        } label: {
            Label("退出 Copi", systemImage: "power")
        }
        .keyboardShortcut("q")
    }

    private var canToggle: Bool {
        processController.isRunning || (processController.status.canStart && settings.snapshot().isRunnable)
    }

    private func toggleProcess() {
        if processController.isRunning {
            processController.stop()
        } else {
            processController.start(settings: settings.snapshot())
        }
    }

    private func openAndFocusWindow(id: String, title: String) {
        openWindow(id: id)
        focusWindow(title: title)
    }

    private func focusWindow(title: String) {
        DispatchQueue.main.async {
            bringWindowToFront(title: title)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) {
            bringWindowToFront(title: title)
        }
    }

    private func bringWindowToFront(title: String) {
        NSApp.activate(ignoringOtherApps: true)
        guard let window = NSApp.windows.first(where: { $0.title == title }) else {
            return
        }
        window.makeKeyAndOrderFront(nil)
        window.orderFrontRegardless()
    }
}
