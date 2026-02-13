// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "SpeechBridge",
    platforms: [
        .macOS(.v26),
    ],
    products: [
        .library(
            name: "SpeechBridge",
            type: .static,
            targets: ["SpeechBridge"]
        ),
    ],
    targets: [
        .target(
            name: "SpeechBridge",
            path: "Sources/SpeechBridge",
            publicHeadersPath: "include",
            swiftSettings: [
                .swiftLanguageMode(.v6),
            ],
            linkerSettings: [
                .linkedFramework("Speech"),
                .linkedFramework("AVFoundation"),
                .linkedFramework("Foundation"),
            ]
        ),
    ]
)
