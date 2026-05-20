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

        Settings {
            SettingsView()
                .environmentObject(settings)
                .environmentObject(processController)
                .frame(width: 560, height: 420)
                .padding()
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
    }
}

private struct MenuBarContentView: View {
    @EnvironmentObject private var settings: SettingsStore
    @EnvironmentObject private var processController: CopiProcessController

    var body: some View {
        Text(processController.status.title)
            .font(.headline)

        Text(settings.mode.title)
            .foregroundStyle(.secondary)

        if let event = processController.lastEvent {
            Divider()
            Text(event.message)
                .lineLimit(2)
        }

        Divider()

        Button {
            toggleProcess()
        } label: {
            Label(processController.isRunning ? "停止同步" : "启动同步", systemImage: processController.isRunning ? "stop.fill" : "play.fill")
        }
        .disabled(!canToggle)

        SettingsLink {
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
}
