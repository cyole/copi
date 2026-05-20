import Foundation

struct ClientSettings {
    var mode: SyncMode
    var cliPath: String
    var deviceName: String
    var deviceID: String
    var relayURL: String
    var accessToken: String
    var syncKey: String

    var secret: String {
        switch mode {
        case .relay:
            return accessToken
        case .lan:
            return syncKey
        }
    }

    var isRunnable: Bool {
        guard !cliPath.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return false
        }
        switch mode {
        case .relay:
            return !relayURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        case .lan:
            return true
        }
    }
}
