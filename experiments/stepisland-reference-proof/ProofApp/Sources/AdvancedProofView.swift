import SwiftUI

struct AdvancedProofView: View {
    let reset: () -> Void

    var body: some View {
        VStack(spacing: 18) {
            Spacer()
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 66, weight: .bold))
                .foregroundStyle(Color(red: 0.20, green: 0.62, blue: 0.36))
            Text("Glyph action executed")
                .font(.system(size: 30, weight: .black, design: .rounded))
                .foregroundStyle(Color(red: 0.025, green: 0.09, blue: 0.22))
            Text("The reference-built CTA is a real SwiftUI Button, not a screenshot hotspot.")
                .font(.system(size: 16, weight: .semibold, design: .rounded))
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
            Button("Back to reference UI", action: reset)
                .buttonStyle(.borderedProminent)
                .accessibilityIdentifier("proof_reset")
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(red: 1.0, green: 0.976, blue: 0.94))
        .accessibilityIdentifier("proof_advanced_screen")
    }
}
