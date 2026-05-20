import SwiftUI

private enum SidebarItem: String, CaseIterable, Identifiable {
    case sync
    case devices
    case logs

    var id: String { rawValue }

    var title: String {
        switch self {
        case .sync:
            return "同步"
        case .devices:
            return "设备"
        case .logs:
            return "日志"
        }
    }

    var systemImage: String {
        switch self {
        case .sync:
            return "arrow.triangle.2.circlepath"
        case .devices:
            return "desktopcomputer"
        case .logs:
            return "list.bullet.rectangle"
        }
    }
}

struct ContentView: View {
    @EnvironmentObject private var settings: SettingsStore
    @EnvironmentObject private var processController: CopiProcessController
    @SceneStorage("selectedSidebarItem") private var selectedItem: SidebarItem = .sync

    var body: some View {
        NavigationSplitView {
            List(SidebarItem.allCases, selection: $selectedItem) { item in
                Label(item.title, systemImage: item.systemImage)
                    .tag(item)
            }
            .listStyle(.sidebar)
            .navigationTitle("Copi")
        } detail: {
            detailView
                .navigationTitle(selectedItem.title)
        }
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    toggleProcess()
                } label: {
                    Label(processController.isRunning ? "停止" : "启动", systemImage: processController.isRunning ? "stop.fill" : "play.fill")
                }
                .disabled(!canToggle)
                .keyboardShortcut("r", modifiers: [.command])
            }
        }
    }

    @ViewBuilder
    private var detailView: some View {
        switch selectedItem {
        case .sync:
            SyncView()
        case .devices:
            DevicesView()
        case .logs:
            LogsView()
        }
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
}
