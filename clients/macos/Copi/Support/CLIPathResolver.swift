import Foundation

enum CLIPathResolver {
    static func defaultPath() -> String {
        if let bundledCLI = Bundle.main.url(forResource: "copi", withExtension: nil),
           FileManager.default.isExecutableFile(atPath: bundledCLI.path) {
            return bundledCLI.path
        }

        let environment = ProcessInfo.processInfo.environment
        if let cliPath = environment["COPI_CLI_PATH"], !cliPath.isEmpty {
            return cliPath
        }
        if let repoRoot = environment["COPI_REPO_ROOT"], !repoRoot.isEmpty {
            return URL(fileURLWithPath: repoRoot).appendingPathComponent("bin/copi").path
        }

        var current = Bundle.main.bundleURL
        for _ in 0..<12 {
            let candidate = current.appendingPathComponent("bin/copi").path
            if FileManager.default.isExecutableFile(atPath: candidate) {
                return candidate
            }
            current.deleteLastPathComponent()
        }

        return "copi"
    }
}
