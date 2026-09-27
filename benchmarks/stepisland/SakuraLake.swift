import SwiftUI

struct SakuraLake: View {
    var width: CGFloat = 1290
    var height: CGFloat = 2796

    var body: some View {
        let centerX = width / 2
        let lakeX: CGFloat = 645
        let lakeY: CGFloat = 1660

        ZStack(alignment: .topLeading) {
            Color(red: 0.867, green: 0.937, blue: 0.945)

            Path { p in
                p.move(to: CGPoint(x: 0, y: 1060))
                p.addCurve(to: CGPoint(x: 510, y: 720), control1: CGPoint(x: 180, y: 810), control2: CGPoint(x: 340, y: 920))
                p.addCurve(to: CGPoint(x: 960, y: 930), control1: CGPoint(x: 670, y: 900), control2: CGPoint(x: 810, y: 770))
                p.addCurve(to: CGPoint(x: 1290, y: 820), control1: CGPoint(x: 1090, y: 800), control2: CGPoint(x: 1200, y: 880))
                p.addLine(to: CGPoint(x: 1290, y: 1550)); p.addLine(to: CGPoint(x: 0, y: 1550)); p.closeSubpath()
            }.fill(Color(red: 0.686, green: 0.784, blue: 0.769)).accessibilityIdentifier("mountains_mid")

            Ellipse().fill(Color(red: 0.447, green: 0.714, blue: 0.784))
                .frame(width: 1040, height: 540).position(x: lakeX, y: lakeY).accessibilityIdentifier("lake")

            RoundedRectangle(cornerRadius: 28).fill(Color(red: 0.773, green: 0.549, blue: 0.408))
                .frame(width: 430, height: 56).position(x: lakeX, y: lakeY - 70).accessibilityIdentifier("bridge")

            Ellipse().fill(Color(red: 0.243, green: 0.333, blue: 0.345))
                .frame(width: 184, height: 216).position(x: centerX, y: 1390).accessibilityIdentifier("mascot")

            Ellipse().fill(Color(red: 0.918, green: 0.651, blue: 0.698))
                .frame(width: 370, height: 620).position(x: 195, y: 2080).accessibilityIdentifier("tree_left")

            Ellipse().fill(Color(red: 0.953, green: 0.757, blue: 0.788))
                .frame(width: 300, height: 530).position(x: 1090, y: 2110).accessibilityIdentifier("tree_right")

            Path { p in
                p.move(to: CGPoint(x: 0, y: 2200))
                p.addCurve(to: CGPoint(x: 560, y: 2180), control1: CGPoint(x: 220, y: 2100), control2: CGPoint(x: 370, y: 2290))
                p.addCurve(to: CGPoint(x: 1080, y: 2150), control1: CGPoint(x: 760, y: 2070), control2: CGPoint(x: 910, y: 2250))
                p.addCurve(to: CGPoint(x: 1290, y: 2080), control1: CGPoint(x: 1180, y: 2090), control2: CGPoint(x: 1240, y: 2110))
                p.addLine(to: CGPoint(x: 1290, y: 2796)); p.addLine(to: CGPoint(x: 0, y: 2796)); p.closeSubpath()
            }.fill(Color(red: 0.408, green: 0.557, blue: 0.439)).accessibilityIdentifier("foreground")

            Text("Give the next few months a shape.")
                .font(.system(size: 58, weight: .semibold, design: .serif))
                .foregroundStyle(Color(red: 0.161, green: 0.208, blue: 0.220))
                .position(x: centerX, y: 220).accessibilityIdentifier("title")
        }
        .frame(width: width, height: height)
        .clipped()
    }
}
