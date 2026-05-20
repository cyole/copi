import SwiftUI

struct LogsView: View {
    @EnvironmentObject private var processController: CopiProcessController

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("\(processController.logs.count) 条")
                    .foregroundStyle(.secondary)
                Spacer()
                Button {
                    processController.clearLogs()
                } label: {
                    Label("清空", systemImage: "trash")
                }
                .disabled(processController.logs.isEmpty)
            }
            .padding([.horizontal, .top], 20)
            .padding(.bottom, 12)

            List {
                ForEach(processController.logs.reversed()) { event in
                    VStack(alignment: .leading, spacing: 5) {
                        HStack(spacing: 8) {
                            Text(event.level.uppercased())
                                .font(.caption2)
                                .fontWeight(.semibold)
                                .foregroundStyle(color(for: event.level))
                            Text(event.eventType)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            Spacer()
                            Text(event.source.rawValue)
                                .font(.caption2)
                                .foregroundStyle(.tertiary)
                        }

                        Text(event.message)
                            .font(.body)
                            .lineLimit(3)
                    }
                    .padding(.vertical, 5)
                }
            }
            .listStyle(.inset)
        }
    }

    private func color(for level: String) -> Color {
        switch level.lowercased() {
        case "error":
            return .red
        case "warn", "warning":
            return .orange
        default:
            return .secondary
        }
    }
}
