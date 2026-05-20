import Combine
import Foundation

@MainActor
final class CopiProcessController: ObservableObject {
    @Published private(set) var status: ProcessStatus = .stopped
    @Published private(set) var logs: [CopiLogEvent] = []
    @Published private(set) var lastEvent: CopiLogEvent?

    private var process: Process?
    private var stdoutPipe: Pipe?
    private var stderrPipe: Pipe?
    private var stdoutBuffer = ""
    private var stderrBuffer = ""

    var isRunning: Bool {
        status.isRunning
    }

    func start(settings: ClientSettings) {
        guard status.canStart else {
            return
        }
        guard settings.isRunnable else {
            fail("请补全运行参数")
            return
        }

        status = .starting

        let process = Process()
        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        let launch = launchConfiguration(settings: settings)

        process.executableURL = launch.executableURL
        process.arguments = launch.arguments
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe
        process.terminationHandler = { [weak self] terminatedProcess in
            Task { @MainActor in
                self?.handleTermination(terminatedProcess.terminationStatus)
            }
        }

        attach(stdoutPipe, source: .stdout)
        attach(stderrPipe, source: .stderr)

        do {
            try process.run()
            self.process = process
            self.stdoutPipe = stdoutPipe
            self.stderrPipe = stderrPipe
            status = .running(pid: process.processIdentifier)
        } catch {
            cleanupPipes()
            fail(error.localizedDescription)
        }
    }

    func stop() {
        guard let process else {
            status = .stopped
            return
        }
        guard process.isRunning else {
            cleanupProcess()
            status = .stopped
            return
        }
        status = .stopping
        process.terminate()
    }

    func clearLogs() {
        logs.removeAll()
        lastEvent = nil
    }

    private func launchConfiguration(settings: ClientSettings) -> (executableURL: URL, arguments: [String]) {
        var arguments = ["client"]
        switch settings.mode {
        case .relay:
            arguments.append(contentsOf: ["--relay", settings.relayURL])
        case .lan:
            arguments.append("--lan")
        }

        if !settings.secret.isEmpty {
            arguments.append(contentsOf: ["--token", settings.secret])
        }
        arguments.append(contentsOf: [
            "--id", settings.deviceID,
            "--name", settings.deviceName,
            "--log-format", "json",
        ])

        if settings.cliPath.contains("/") {
            return (URL(fileURLWithPath: settings.cliPath), arguments)
        }
        return (URL(fileURLWithPath: "/usr/bin/env"), [settings.cliPath] + arguments)
    }

    private func attach(_ pipe: Pipe, source: LogSource) {
        pipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty, let text = String(data: data, encoding: .utf8) else {
                return
            }
            Task { @MainActor in
                self?.consume(text, source: source)
            }
        }
    }

    private func consume(_ text: String, source: LogSource) {
        switch source {
        case .stdout:
            appendCompleteLines(from: text, buffer: &stdoutBuffer, source: source)
        case .stderr:
            appendCompleteLines(from: text, buffer: &stderrBuffer, source: source)
        }
    }

    private func appendCompleteLines(from text: String, buffer: inout String, source: LogSource) {
        buffer += text
        let parts = buffer.components(separatedBy: "\n")

        if buffer.hasSuffix("\n") {
            buffer = ""
            parts.dropLast().forEach { appendLine($0, source: source) }
        } else {
            buffer = parts.last ?? ""
            parts.dropLast().forEach { appendLine($0, source: source) }
        }
    }

    private func appendLine(_ line: String, source: LogSource) {
        let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return
        }

        let event = CopiLogEvent(line: trimmed, source: source)
        logs.append(event)
        if logs.count > 200 {
            logs.removeFirst(logs.count - 200)
        }
        lastEvent = event
    }

    private func handleTermination(_ terminationStatus: Int32) {
        cleanupProcess()
        if terminationStatus == 0 || status == .stopping {
            status = .stopped
        } else {
            fail("copi exited with status \(terminationStatus)")
        }
    }

    private func fail(_ message: String) {
        appendLine(message, source: .stderr)
        status = .failed(message)
    }

    private func cleanupProcess() {
        cleanupPipes()
        process = nil
    }

    private func cleanupPipes() {
        stdoutPipe?.fileHandleForReading.readabilityHandler = nil
        stderrPipe?.fileHandleForReading.readabilityHandler = nil
        stdoutPipe = nil
        stderrPipe = nil
        stdoutBuffer = ""
        stderrBuffer = ""
    }
}
