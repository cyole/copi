import Foundation

enum SyncMode: String, CaseIterable, Identifiable {
    case relay
    case lan

    var id: String { rawValue }

    var title: String {
        switch self {
        case .relay:
            return "中转模式"
        case .lan:
            return "局域网"
        }
    }

    var systemImage: String {
        switch self {
        case .relay:
            return "server.rack"
        case .lan:
            return "network"
        }
    }
}
