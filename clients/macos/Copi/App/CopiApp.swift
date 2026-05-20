import AppKit
import SwiftUI

@main
struct CopiApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var settings = SettingsStore()
    @StateObject private var processController = CopiProcessController()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(settings)
                .environmentObject(processController)
                .frame(minWidth: 860, minHeight: 560)
        }
        .windowResizability(.contentMinSize)

        Settings {
            SettingsView()
                .environmentObject(settings)
                .environmentObject(processController)
                .frame(width: 560)
                .padding()
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
    }
}
