import Foundation

enum ProcessStatus: Equatable {
    case stopped
    case starting
    case running(pid: Int32)
    case stopping
    case failed(String)

    var title: String {
        switch self {
        case .stopped:
            return "已停止"
        case .starting:
            return "启动中"
        case .running(let pid):
            return "运行中 · \(pid)"
        case .stopping:
            return "停止中"
        case .failed:
            return "启动失败"
        }
    }

    var isRunning: Bool {
        if case .running = self {
            return true
        }
        return false
    }

    var canStart: Bool {
        switch self {
        case .stopped, .failed:
            return true
        case .starting, .running, .stopping:
            return false
        }
    }
}
