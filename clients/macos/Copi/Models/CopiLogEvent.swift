import Foundation

enum LogSource: String {
    case stdout
    case stderr
}

struct CopiLogEvent: Identifiable {
    let id = UUID()
    let receivedAt = Date()
    let source: LogSource
    let level: String
    let eventType: String
    let message: String
    let rawLine: String

    init(line: String, source: LogSource) {
        self.source = source
        self.rawLine = line

        guard
            let data = line.data(using: .utf8),
            let object = try? JSONSerialization.jsonObject(with: data),
            let payload = object as? [String: Any]
        else {
            self.level = source == .stderr ? "error" : "info"
            self.eventType = source.rawValue
            self.message = line
            return
        }

        self.level = payload["level"] as? String ?? "info"
        self.eventType = payload["type"] as? String ?? "event"
        self.message = payload["message"] as? String ?? line
    }
}
