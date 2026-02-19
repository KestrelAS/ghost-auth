// swift-tools-version:5.3

import PackageDescription

let package = Package(
  name: "tauri-plugin-share-file",
  platforms: [
    .iOS(.v14),
  ],
  products: [
    .library(
      name: "tauri-plugin-share-file",
      type: .static,
      targets: ["tauri-plugin-share-file"])
  ],
  dependencies: [
    .package(name: "Tauri", path: "../.tauri/tauri-api")
  ],
  targets: [
    .target(
      name: "tauri-plugin-share-file",
      dependencies: [
        .byName(name: "Tauri")
      ],
      path: "Sources")
  ]
)
