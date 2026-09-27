// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "GlyphStudio",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "GlyphStudio", targets: ["GlyphStudio"]),
        .executable(name: "GlyphVision", targets: ["GlyphVision"]),
    ],
    targets: [
        .executableTarget(name: "GlyphStudio"),
        .executableTarget(name: "GlyphVision"),
        .testTarget(name: "GlyphStudioTests", dependencies: ["GlyphStudio"]),
    ]
)
