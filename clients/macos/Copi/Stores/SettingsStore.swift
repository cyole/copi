import Combine
import Foundation
import ServiceManagement

@MainActor
final class SettingsStore: ObservableObject {
    private enum Keys {
        static let mode = "mode"
        static let cliPath = "cliPath"
        static let deviceName = "deviceName"
        static let deviceID = "deviceID"
        static let relayURL = "relayURL"
        static let accessToken = "accessToken"
        static let syncKey = "syncKey"
        static let launchAtLogin = "launchAtLogin"
    }

    private let defaults: UserDefaults

    @Published var mode: SyncMode { didSet { save() } }
    @Published var cliPath: String { didSet { save() } }
    @Published var deviceName: String { didSet { save() } }
    @Published var deviceID: String { didSet { save() } }
    @Published var relayURL: String { didSet { save() } }
    @Published var accessToken: String { didSet { save() } }
    @Published var syncKey: String { didSet { save() } }
    @Published var launchAtLogin: Bool {
        didSet {
            save()
            applyLaunchAtLogin()
        }
    }
    @Published private(set) var launchAtLoginError: String?

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        self.mode = SyncMode(rawValue: defaults.string(forKey: Keys.mode) ?? "") ?? .relay
        self.cliPath = defaults.string(forKey: Keys.cliPath) ?? CLIPathResolver.defaultPath()
        self.deviceName = defaults.string(forKey: Keys.deviceName) ?? Host.current().localizedName ?? "Mac"
        self.deviceID = defaults.string(forKey: Keys.deviceID) ?? SecretGenerator.deviceID()
        self.relayURL = defaults.string(forKey: Keys.relayURL) ?? "http://127.0.0.1:9527"
        self.accessToken = defaults.string(forKey: Keys.accessToken) ?? ""
        self.syncKey = defaults.string(forKey: Keys.syncKey) ?? SecretGenerator.syncKey()
        self.launchAtLogin = defaults.bool(forKey: Keys.launchAtLogin)
    }

    func snapshot() -> ClientSettings {
        ClientSettings(
            mode: mode,
            cliPath: cliPath,
            deviceName: deviceName,
            deviceID: deviceID,
            relayURL: relayURL,
            accessToken: accessToken,
            syncKey: syncKey
        )
    }

    func generateDeviceID() {
        deviceID = SecretGenerator.deviceID()
    }

    func generateSyncKey() {
        syncKey = SecretGenerator.syncKey()
    }

    private func save() {
        defaults.set(mode.rawValue, forKey: Keys.mode)
        defaults.set(cliPath, forKey: Keys.cliPath)
        defaults.set(deviceName, forKey: Keys.deviceName)
        defaults.set(deviceID, forKey: Keys.deviceID)
        defaults.set(relayURL, forKey: Keys.relayURL)
        defaults.set(accessToken, forKey: Keys.accessToken)
        defaults.set(syncKey, forKey: Keys.syncKey)
        defaults.set(launchAtLogin, forKey: Keys.launchAtLogin)
    }

    private func applyLaunchAtLogin() {
        do {
            if launchAtLogin {
                if SMAppService.mainApp.status != .enabled {
                    try SMAppService.mainApp.register()
                }
            } else {
                if SMAppService.mainApp.status == .enabled {
                    try SMAppService.mainApp.unregister()
                }
            }
            launchAtLoginError = nil
        } catch {
            launchAtLoginError = error.localizedDescription
        }
    }
}
