import SwiftUI

struct StatusHeaderView: View {
    @EnvironmentObject private var settings: SettingsStore
    @EnvironmentObject private var processController: CopiProcessController

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .center, spacing: 12) {
                Image(systemName: settings.mode.systemImage)
                    .font(.title2)
                    .foregroundStyle(.secondary)
                    .frame(width: 28)

                VStack(alignment: .leading, spacing: 3) {
                    Text(settings.mode.title)
                        .font(.headline)
                    Text(processController.status.title)
                        .foregroundStyle(statusColor)
                }

                Spacer()

                statusDot
            }

            if let event = processController.lastEvent {
                Divider()
                HStack(spacing: 8) {
                    Text(event.eventType)
                        .font(.caption)
                        .fontWeight(.medium)
                        .foregroundStyle(.secondary)
                    Text(event.message)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
            }
        }
        .padding(16)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
    }

    private var statusDot: some View {
        Circle()
            .fill(statusColor)
            .frame(width: 10, height: 10)
            .accessibilityLabel(processController.status.title)
    }

    private var statusColor: Color {
        switch processController.status {
        case .stopped:
            return .secondary
        case .starting, .stopping:
            return .orange
        case .running:
            return .green
        case .failed:
            return .red
        }
    }
}
