// swift-tools-version:6.0
import PackageDescription

let package = Package(
    name: "ClairObscur",
    platforms: [.macOS(.v14)],
    targets: [
        .executableTarget(
            name: "ClairObscur",
            path: "Sources/ClairObscur",
            swiftSettings: [.swiftLanguageMode(.v5)]
        )
    ]
)
