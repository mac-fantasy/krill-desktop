// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "KrillDesktop",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "KrillMac", targets: ["KrillMac"]),
        .library(name: "KrillCore", targets: ["KrillCore"])
    ],
    targets: [
        .target(name: "KrillCore"),
        .executableTarget(name: "KrillMac", dependencies: ["KrillCore"]),
        .testTarget(name: "KrillCoreTests", dependencies: ["KrillCore"]),
        .testTarget(name: "KrillMacTests", dependencies: ["KrillMac"])
    ],
    swiftLanguageModes: [.v6]
)
