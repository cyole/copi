import SwiftUI

struct SettingsView: View {
    @EnvironmentObject private var settings: SettingsStore
    @EnvironmentObject private var processController: CopiProcessController

    var body: some View {
        VStack(spacing: 0) {
            header

            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    SettingsSection(title: "同步", systemImage: settings.mode.systemImage) {
                        SettingsRow(title: "模式") {
                            Picker("模式", selection: $settings.mode) {
                                ForEach(SyncMode.allCases) { mode in
                                    Text(mode.title).tag(mode)
                                }
                            }
                            .pickerStyle(.segmented)
                            .labelsHidden()
                        }

                        if settings.mode == .relay {
                            SettingsRow(title: "地址") {
                                TextField("http://127.0.0.1:9527", text: $settings.relayURL)
                                    .textFieldStyle(.roundedBorder)
                            }

                            SettingsRow(title: "访问令牌") {
                                SecureField("", text: $settings.accessToken)
                                    .textFieldStyle(.roundedBorder)
                            }
                        } else {
                            SettingsRow(title: "同步密钥") {
                                HStack(spacing: 8) {
                                    SecureField("", text: $settings.syncKey)
                                        .textFieldStyle(.roundedBorder)

                                    Button {
                                        settings.generateSyncKey()
                                    } label: {
                                        Label("生成", systemImage: "key.fill")
                                    }
                                }
                            }
                        }
                    }
                    .disabled(processController.isRunning)

                    SettingsSection(title: "设备", systemImage: "macbook") {
                        SettingsRow(title: "名称") {
                            TextField("Mac", text: $settings.deviceName)
                                .textFieldStyle(.roundedBorder)
                        }

                        SettingsRow(title: "ID") {
                            HStack(spacing: 8) {
                                TextField("", text: $settings.deviceID)
                                    .textFieldStyle(.roundedBorder)
                                    .font(.system(.body, design: .monospaced))

                                Button {
                                    settings.generateDeviceID()
                                } label: {
                                    Label("生成", systemImage: "sparkles")
                                }
                            }
                        }
                    }
                    .disabled(processController.isRunning)

                    SettingsSection(title: "行为", systemImage: "power") {
                        SettingsRow(title: "启动") {
                            Toggle("登录后自动启动 Copi", isOn: $settings.launchAtLogin)
                                .toggleStyle(.switch)
                        }

                        if let error = settings.launchAtLoginError {
                            SettingsRow(title: "") {
                                Text(error)
                                    .font(.caption)
                                    .foregroundStyle(.red)
                                    .lineLimit(2)
                            }
                        }
                    }
                }
                .padding(22)
            }
        }
        .background(.regularMaterial)
    }

    private var header: some View {
        HStack(spacing: 14) {
            Image(systemName: processController.status.cloudSystemImage)
                .font(.system(size: 30))
                .symbolRenderingMode(.hierarchical)
                .foregroundStyle(Color.accentColor)
                .frame(width: 36, height: 36)

            VStack(alignment: .leading, spacing: 3) {
                Text("Copi")
                    .font(.title3)
                    .fontWeight(.semibold)

                HStack(spacing: 8) {
                    statusDot

                    Text(processController.status.title)
                        .foregroundStyle(.secondary)

                    Text("·")
                        .foregroundStyle(.tertiary)

                    Text(settings.mode.title)
                        .foregroundStyle(.secondary)
                }
                .font(.caption)
            }

            Spacer()

            Button {
                toggleProcess()
            } label: {
                Label(processController.isRunning ? "停止" : "启动", systemImage: processController.isRunning ? "stop.fill" : "play.fill")
                    .frame(minWidth: 72)
            }
            .buttonStyle(.borderedProminent)
            .disabled(!canToggle)
        }
        .padding(.horizontal, 22)
        .padding(.vertical, 18)
    }

    private var canToggle: Bool {
        processController.isRunning || (processController.status.canStart && settings.snapshot().isRunnable)
    }

    private func toggleProcess() {
        if processController.isRunning {
            processController.stop()
        } else {
            processController.start(settings: settings.snapshot())
        }
    }

    private var statusDot: some View {
        Circle()
            .fill(statusColor)
            .frame(width: 8, height: 8)
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

private struct SettingsSection<Content: View>: View {
    var title: String
    var systemImage: String
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label(title, systemImage: systemImage)
                .font(.headline)
                .foregroundStyle(.primary)

            VStack(spacing: 10) {
                content
            }
            .padding(14)
            .background(.background, in: RoundedRectangle(cornerRadius: 8))
        }
    }
}

private struct SettingsRow<Content: View>: View {
    var title: String
    @ViewBuilder var content: Content

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            Text(title)
                .foregroundStyle(.secondary)
                .frame(width: 72, alignment: .trailing)

            content
                .frame(maxWidth: .infinity)
        }
    }
}
