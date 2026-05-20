import Foundation

enum SecretGenerator {
    static func syncKey(byteCount: Int = 24) -> String {
        let bytes = (0..<byteCount).map { _ in UInt8.random(in: UInt8.min...UInt8.max) }
        return Data(bytes).base64EncodedString()
    }

    static func deviceID() -> String {
        "mac-" + UUID().uuidString.lowercased()
    }
}
