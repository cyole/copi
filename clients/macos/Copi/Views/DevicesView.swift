import SwiftUI

struct DevicesView: View {
    @EnvironmentObject private var settings: SettingsStore
    @EnvironmentObject private var processController: CopiProcessController

    var body: some View {
        List {
            Section("本机") {
                DeviceRow(
                    title: settings.deviceName,
                    subtitle: settings.deviceID,
                    systemImage: "macbook",
                    status: processController.status.title
                )
            }

            Section("同步方式") {
                DeviceRow(
                    title: settings.mode.title,
                    subtitle: settings.mode == .relay ? settings.relayURL : "LAN",
                    systemImage: settings.mode.systemImage,
                    status: processController.isRunning ? "已启用" : "未启用"
                )
            }
        }
        .listStyle(.inset)
        .padding(20)
    }
}

private struct DeviceRow: View {
    var title: String
    var subtitle: String
    var systemImage: String
    var status: String

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: systemImage)
                .foregroundStyle(.secondary)
                .frame(width: 24)

            VStack(alignment: .leading, spacing: 3) {
                Text(title)
                    .font(.headline)
                    .lineLimit(1)
                Text(subtitle)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

            Spacer()

            Text(status)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(.vertical, 6)
    }
}
