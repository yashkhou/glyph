import SwiftUI

private enum ProofPalette {
    static let navy = Color(red: 0.025, green: 0.09, blue: 0.22)
    static let body = Color(red: 0.33, green: 0.43, blue: 0.53)
    static let paper = Color(red: 0.995, green: 0.976, blue: 0.94)
    static let sky = Color(red: 0.25, green: 0.61, blue: 0.96)
    static let coral = Color(red: 1.0, green: 0.34, blue: 0.48)
}

struct ReferenceExactWelcome: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var page = 0
    @State private var proofMessage: String?
    @State private var mascotWalking = false

    var body: some View {
        ZStack {
            if page == 0 {
                welcome
                    .transition(.opacity)
            } else {
                ProofSecondPage(onBack: { withAnimation { page = 0 } })
                    .transition(.opacity.combined(with: .scale(scale: 0.985)))
            }
        }
        .animation(reduceMotion ? .easeOut(duration: 0.12) : .spring(response: 0.42, dampingFraction: 0.9), value: page)
    }

    private var welcome: some View {
        GeometryReader { proxy in
            let w = proxy.size.width
            let h = proxy.size.height
            ZStack {
                Image("Background")
                    .resizable()
                    .scaledToFill()
                    .frame(width: w, height: h)
                    .clipped()
                    .ignoresSafeArea()

                LinearGradient(
                    colors: [.clear, .clear, ProofPalette.paper.opacity(0.88), ProofPalette.paper],
                    startPoint: .top,
                    endPoint: .bottom
                )
                .frame(height: h * 0.31)
                .frame(maxHeight: .infinity, alignment: .bottom)
                .ignoresSafeArea(edges: .bottom)

                heroCopy(width: w)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)

                DawnMascot(walking: mascotWalking)
                    .frame(width: w * 0.19, height: w * 0.19)
                    .position(x: w * 0.43, y: h * 0.61)
                    .accessibilityLabel("StepIsland companion walking")

                featureRow(width: w)
                    .frame(width: w * 0.86)
                    .position(x: w / 2, y: h * 0.81)

                Button {
                    withAnimation { page = 1 }
                } label: {
                    HStack {
                        Spacer()
                        Text("Get started")
                            .font(.system(size: w * 0.038, weight: .bold, design: .rounded))
                        Spacer()
                        Image(systemName: "arrow.right")
                            .font(.system(size: w * 0.036, weight: .bold))
                    }
                    .foregroundStyle(.white)
                    .padding(.horizontal, w * 0.045)
                    .frame(height: h * 0.058)
                    .background(ProofPalette.navy, in: Capsule())
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("glyph-proof-get-started")
                .frame(width: w * 0.82)
                .position(x: w / 2, y: h * 0.915)

                pagination
                    .position(x: w / 2, y: h * 0.963)

                if let proofMessage {
                    Text(proofMessage)
                        .font(.system(size: 13, weight: .bold, design: .rounded))
                        .foregroundStyle(ProofPalette.navy)
                        .padding(.horizontal, 14)
                        .padding(.vertical, 9)
                        .background(.ultraThinMaterial, in: Capsule())
                        .position(x: w / 2, y: h * 0.48)
                        .transition(.opacity.combined(with: .scale))
                }
            }
            .ignoresSafeArea()
            .onAppear {
                guard !reduceMotion else { return }
                mascotWalking = true
            }
        }
    }

    private func heroCopy(width: CGFloat) -> some View {
        VStack(alignment: .leading, spacing: -6) {
            Text("Walking, with\nsomewhere to go.")
                .font(.system(size: min(44, width * 0.102), weight: .black, design: .rounded))
                .tracking(-1.2)
                .foregroundStyle(ProofPalette.navy)
                .lineLimit(2)
                .minimumScaleFactor(0.72)
                .allowsTightening(true)
                .lineSpacing(-4)
            Text("Your real steps wake up a tiny world\nand keep progress close.")
                .font(.system(size: min(18, width * 0.043), weight: .semibold, design: .rounded))
                .foregroundStyle(ProofPalette.body)
                .lineSpacing(2)
        }
        .frame(maxWidth: width * 0.82, alignment: .leading)
        .padding(.leading, width * 0.10)
        .padding(.top, 38)
    }

    private func featureRow(width: CGFloat) -> some View {
        HStack(alignment: .top, spacing: 0) {
            featureButton(
                title: "Apple Health",
                detail: "Your real steps",
                message: "Apple Health is a real native action."
            ) { HealthFeatureIcon() }

            featureButton(
                title: "Live Activity",
                detail: "Progress at a glance",
                message: "Live Activity is a real native action."
            ) { LiveActivityFeatureIcon() }

            featureButton(
                title: "Friend walks",
                detail: "More fun together",
                message: "Friend walks is a real native action."
            ) { FriendWalksFeatureIcon() }
        }
    }

    private func featureButton<Icon: View>(
        title: String,
        detail: String,
        message: String,
        @ViewBuilder icon: () -> Icon
    ) -> some View {
        Button {
            withAnimation { proofMessage = message }
            Task {
                try? await Task.sleep(for: .seconds(1.15))
                withAnimation { proofMessage = nil }
            }
        } label: {
            VStack(spacing: 5) {
                icon()
                    .frame(width: 54, height: 48)
                Text(title)
                    .font(.system(size: 10.5, weight: .black, design: .rounded))
                    .foregroundStyle(ProofPalette.navy)
                    .lineLimit(1)
                    .minimumScaleFactor(0.8)
                Text(detail)
                    .font(.system(size: 9, weight: .semibold, design: .rounded))
                    .foregroundStyle(ProofPalette.body)
                    .lineLimit(1)
                    .minimumScaleFactor(0.7)
            }
            .frame(maxWidth: .infinity)
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("glyph-proof-\(title.lowercased().replacingOccurrences(of: " ", with: "-"))")
    }

    private var pagination: some View {
        HStack(spacing: 12) {
            ForEach(0..<6, id: \.self) { index in
                Circle()
                    .fill(index == 0 ? ProofPalette.sky : Color.gray.opacity(0.18))
                    .frame(width: 7, height: 7)
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Step 1 of 6")
    }
}

private struct DawnMascot: View {
    let walking: Bool
    @State private var phase = false

    var body: some View {
        Image("DawnMascot")
            .resizable()
            .renderingMode(.original)
            .scaledToFit()
            .rotationEffect(.degrees(walking && phase ? -2.5 : 1.5))
            .offset(y: walking && phase ? -4 : 2)
            .onAppear {
                guard walking else { return }
                withAnimation(.easeInOut(duration: 0.68).repeatForever(autoreverses: true)) {
                    phase = true
                }
            }
    }
}

private struct BlobBody: Shape {
    func path(in rect: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: rect.midX, y: rect.minY + rect.height * 0.08))
        p.addCurve(to: CGPoint(x: rect.maxX * 0.88, y: rect.height * 0.72),
                   control1: CGPoint(x: rect.maxX * 0.79, y: rect.minY),
                   control2: CGPoint(x: rect.maxX * 0.98, y: rect.height * 0.38))
        p.addQuadCurve(to: CGPoint(x: rect.maxX * 0.72, y: rect.height * 0.91),
                       control: CGPoint(x: rect.maxX * 0.88, y: rect.height * 0.86))
        p.addQuadCurve(to: CGPoint(x: rect.maxX * 0.60, y: rect.height * 0.90),
                       control: CGPoint(x: rect.maxX * 0.67, y: rect.maxY))
        p.addQuadCurve(to: CGPoint(x: rect.maxX * 0.50, y: rect.height * 0.94),
                       control: CGPoint(x: rect.maxX * 0.56, y: rect.height * 0.91))
        p.addQuadCurve(to: CGPoint(x: rect.maxX * 0.39, y: rect.height * 0.90),
                       control: CGPoint(x: rect.maxX * 0.44, y: rect.height * 0.91))
        p.addQuadCurve(to: CGPoint(x: rect.maxX * 0.27, y: rect.height * 0.91),
                       control: CGPoint(x: rect.maxX * 0.33, y: rect.maxY))
        p.addQuadCurve(to: CGPoint(x: rect.maxX * 0.12, y: rect.height * 0.72),
                       control: CGPoint(x: rect.maxX * 0.12, y: rect.height * 0.86))
        p.addCurve(to: CGPoint(x: rect.midX, y: rect.minY + rect.height * 0.08),
                   control1: CGPoint(x: rect.maxX * 0.02, y: rect.height * 0.38),
                   control2: CGPoint(x: rect.maxX * 0.21, y: rect.minY))
        p.closeSubpath()
        return p
    }
}

private struct MiniBlobMascot: View {
    var body: some View {
        Image("DawnMascot")
            .resizable()
            .renderingMode(.original)
            .scaledToFit()
    }
}

private struct HealthFeatureIcon: View {
    var body: some View {
        RoundedRectangle(cornerRadius: 17, style: .continuous)
            .fill(Color.white.opacity(0.94))
            .overlay {
                Image(systemName: "heart.fill")
                    .font(.system(size: 22, weight: .bold))
                    .foregroundStyle(ProofPalette.coral)
            }
            .frame(width: 46, height: 46)
    }
}

private struct LiveActivityFeatureIcon: View {
    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(Color.white.opacity(0.92))
            HStack(spacing: 4) {
                MiniBlobMascot()
                    .frame(width: 17, height: 20)
                VStack(alignment: .leading, spacing: 2) {
                    Text("2,246")
                        .font(.system(size: 8.5, weight: .black, design: .rounded))
                        .foregroundStyle(.white)
                    Capsule()
                        .fill(Color.white.opacity(0.18))
                        .frame(width: 25, height: 3)
                }
            }
            .frame(width: 50, height: 27)
            .background(Color.black.opacity(0.94), in: Capsule())
        }
        .frame(width: 54, height: 44)
    }
}

private struct FriendWalksFeatureIcon: View {
    var body: some View {
        ZStack {
            Circle().fill(Color.white.opacity(0.88))
            HStack(spacing: -6) {
                Image("RockMascot")
                    .resizable().scaledToFit().frame(width: 30, height: 30)
                Image("FlowerMascot")
                    .resizable().scaledToFit().frame(width: 32, height: 32)
            }
        }
        .frame(width: 48, height: 48)
    }
}

private struct ProofSecondPage: View {
    let onBack: () -> Void

    var body: some View {
        ZStack {
            ProofPalette.paper.ignoresSafeArea()
            VStack(spacing: 20) {
                Spacer()
                BlobMascot(walking: true)
                    .frame(width: 108, height: 122)
                Text("Your steps go in.\nThe island wakes up.")
                    .font(.system(size: 36, weight: .black, design: .rounded))
                    .tracking(-1.0)
                    .foregroundStyle(ProofPalette.navy)
                    .multilineTextAlignment(.center)
                    .accessibilityIdentifier("glyph-proof-second-page")
                Text("This is a real second SwiftUI state, not another screenshot.")
                    .font(.system(size: 15, weight: .semibold, design: .rounded))
                    .foregroundStyle(ProofPalette.body)
                    .multilineTextAlignment(.center)
                Button("Back to reference screen", action: onBack)
                    .font(.system(size: 16, weight: .bold, design: .rounded))
                    .foregroundStyle(.white)
                    .frame(maxWidth: .infinity, minHeight: 54)
                    .background(ProofPalette.navy, in: Capsule())
                    .padding(.horizontal, 34)
                    .accessibilityIdentifier("glyph-proof-back")
                Spacer()
            }
        }
    }
}
