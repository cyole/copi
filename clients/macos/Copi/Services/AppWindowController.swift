import AppKit
import SwiftUI

@MainActor
final class AppWindowController: ObservableObject {
    private var settingsWindow: NSWindow?
    private var logsWindow: NSWindow?

    func showSettings(settings: SettingsStore, processController: CopiProcessController) {
        if settingsWindow == nil {
            let rootView = SettingsView()
                .environmentObject(settings)
                .environmentObject(processController)
                .frame(width: 560, height: 420)
                .padding()

            settingsWindow = makeWindow(
                title: "设置",
                size: NSSize(width: 600, height: 480),
                rootView: rootView
            )
        }

        show(settingsWindow)
    }

    func showLogs(processController: CopiProcessController) {
        if logsWindow == nil {
            let rootView = LogsView()
                .environmentObject(processController)
                .frame(minWidth: 680, minHeight: 420)

            logsWindow = makeWindow(
                title: "日志",
                size: NSSize(width: 760, height: 520),
                rootView: rootView
            )
        }

        show(logsWindow)
    }

    private func makeWindow<Content: View>(title: String, size: NSSize, rootView: Content) -> NSWindow {
        let window = NSWindow(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = title
        window.contentViewController = NSHostingController(rootView: rootView)
        window.isReleasedWhenClosed = false
        window.collectionBehavior = [.managed, .moveToActiveSpace]
        window.center()
        return window
    }

    private func show(_ window: NSWindow?) {
        guard let window else {
            return
        }

        bringToFront(window)
        DispatchQueue.main.async { [weak self, weak window] in
            self?.bringToFront(window)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) { [weak self, weak window] in
            self?.bringToFront(window)
        }
    }

    private func bringToFront(_ window: NSWindow?) {
        guard let window else {
            return
        }

        NSApp.activate(ignoringOtherApps: true)
        window.deminiaturize(nil)
        window.makeKeyAndOrderFront(nil)
        window.orderFrontRegardless()
    }
}
