import SwiftUI

struct SettingsView: View {
    @EnvironmentObject private var settings: SettingsStore
    @EnvironmentObject private var processController: CopiProcessController

    var body: some View {
        Form {
            Section("运行") {
                Picker("模式", selection: $settings.mode) {
                    ForEach(SyncMode.allCases) { mode in
                        Label(mode.title, systemImage: mode.systemImage)
                            .tag(mode)
                    }
                }
                .pickerStyle(.segmented)

                TextField("CLI", text: $settings.cliPath)
                    .textFieldStyle(.roundedBorder)
            }

            Section("设备") {
                TextField("名称", text: $settings.deviceName)
                    .textFieldStyle(.roundedBorder)

                HStack {
                    TextField("ID", text: $settings.deviceID)
                        .textFieldStyle(.roundedBorder)
                    Button {
                        settings.generateDeviceID()
                    } label: {
                        Label("生成", systemImage: "sparkles")
                    }
                }
            }

            if settings.mode == .relay {
                Section("中转服务") {
                    TextField("地址", text: $settings.relayURL)
                        .textFieldStyle(.roundedBorder)
                    SecureField("访问令牌", text: $settings.accessToken)
                        .textFieldStyle(.roundedBorder)
                }
            } else {
                Section("局域网") {
                    HStack {
                        SecureField("同步密钥", text: $settings.syncKey)
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
    }
}
