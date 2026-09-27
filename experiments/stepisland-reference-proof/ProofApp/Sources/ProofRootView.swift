import SwiftUI

struct ProofRootView: View {
    @State private var advanced = false

    var body: some View {
        GeometryReader { proxy in
            ZStack {
                Color(red: 1.0, green: 0.976, blue: 0.94)
                    .ignoresSafeArea()

                if advanced {
                    AdvancedProofView {
                        advanced = false
                    }
                    .transition(.opacity.combined(with: .move(edge: .trailing)))
                } else {
                    let scale = min(proxy.size.width / 430, proxy.size.height / 932)
                    GlyphSceneView { action in
                        guard action == "advance_onboarding" else { return }
                        withAnimation(.spring(response: 0.38, dampingFraction: 0.88)) {
                            advanced = true
                        }
                    }
                    .scaleEffect(scale, anchor: .center)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .transition(.opacity)
                }
            }
        }
        .preferredColorScheme(.light)
    }
}
