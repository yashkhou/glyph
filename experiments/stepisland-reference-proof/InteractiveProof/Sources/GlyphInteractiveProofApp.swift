import SwiftUI

@main
struct GlyphInteractiveProofApp: App {
    var body: some Scene {
        WindowGroup {
            ReferenceExactWelcome()
                .preferredColorScheme(.light)
                .statusBarHidden(true)
        }
    }
}
