// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "LlmBridge",
    platforms: [
        .macOS(.v26),
    ],
    products: [
        .library(
            name: "LlmBridge",
            type: .static,
            targets: ["LlmBridge"]
        ),
    ],
    targets: [
        .target(
            name: "LlmBridge",
            path: "Sources/LlmBridge",
            publicHeadersPath: "include",
            swiftSettings: [
                .swiftLanguageMode(.v6),
            ],
            linkerSettings: [
                .linkedFramework("Foundation"),
            ]
        ),
    ]
)
