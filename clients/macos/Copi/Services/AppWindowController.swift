import AppKit
import SwiftUI

@MainActor
final class AppWindowController: ObservableObject {
    private var settingsWindow: NSWindow?
    private var logsWindow: NSWindow?
    private var didCenterSettingsWindow = false
    private var didCenterLogsWindow = false

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

        show(settingsWindow, centerOnFirstShow: !didCenterSettingsWindow)
        didCenterSettingsWindow = true
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

        show(logsWindow, centerOnFirstShow: !didCenterLogsWindow)
        didCenterLogsWindow = true
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
        return window
    }

    private func show(_ window: NSWindow?, centerOnFirstShow: Bool) {
        guard let window else {
            return
        }

        bringToFront(window)
        DispatchQueue.main.async { [weak self, weak window] in
            self?.bringToFront(window, shouldCenter: centerOnFirstShow)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) { [weak self, weak window] in
            self?.bringToFront(window, shouldCenter: centerOnFirstShow)
        }
    }

    private func bringToFront(_ window: NSWindow?, shouldCenter: Bool = false) {
        guard let window else {
            return
        }

        NSApp.activate(ignoringOtherApps: true)
        if shouldCenter {
            window.contentView?.layoutSubtreeIfNeeded()
            center(window, on: presentationScreen())
        }
        window.deminiaturize(nil)
        window.makeKeyAndOrderFront(nil)
        window.orderFrontRegardless()
    }

    private func center(_ window: NSWindow, on screen: NSScreen?) {
        guard let visibleFrame = screen?.visibleFrame else {
            window.center()
            return
        }

        let frame = window.frame
        let origin = NSPoint(
            x: visibleFrame.midX - frame.width / 2,
            y: visibleFrame.midY - frame.height / 2
        )
        window.setFrameOrigin(origin)
    }

    private func presentationScreen() -> NSScreen? {
        let mouseLocation = NSEvent.mouseLocation
        return NSScreen.screens.first { screen in
            screen.frame.contains(mouseLocation)
        } ?? NSScreen.main
    }
}
