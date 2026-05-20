import SwiftUI

struct SyncView: View {
    @EnvironmentObject private var settings: SettingsStore
    @EnvironmentObject private var processController: CopiProcessController

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                StatusHeaderView()
                SettingsView()
            }
            .padding(24)
            .frame(maxWidth: 760, alignment: .leading)
        }
    }
}
