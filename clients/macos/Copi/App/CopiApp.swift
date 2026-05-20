import AppKit
import SwiftUI

@main
struct CopiApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var settings = SettingsStore()
    @StateObject private var processController = CopiProcessController()
    @StateObject private var windowController = AppWindowController()

    var body: some Scene {
        MenuBarExtra {
            MenuBarContentView()
                .environmentObject(settings)
                .environmentObject(processController)
                .environmentObject(windowController)
        } label: {
            Label("Copi", systemImage: processController.status.cloudSystemImage)
        }
        .menuBarExtraStyle(.menu)
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        TerminationGate.shared.shouldAllowTermination ? .terminateNow : .terminateCancel
    }
}

private struct MenuBarContentView: View {
    @EnvironmentObject private var settings: SettingsStore
    @EnvironmentObject private var processController: CopiProcessController
    @EnvironmentObject private var windowController: AppWindowController

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
            windowController.showLogs(processController: processController)
        } label: {
            Label("日志...", systemImage: "doc.text.magnifyingglass")
        }

        Button {
            windowController.showSettings(settings: settings, processController: processController)
        } label: {
            Label("设置...", systemImage: "gearshape")
        }

        Divider()

        Button {
            processController.stop()
            TerminationGate.shared.shouldAllowTermination = true
            NSApp.terminate(nil)
        } label: {
            Label("退出 Copi", systemImage: "power")
        }
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
}

final class TerminationGate {
    static let shared = TerminationGate()

    var shouldAllowTermination = false

    private init() {}
}
