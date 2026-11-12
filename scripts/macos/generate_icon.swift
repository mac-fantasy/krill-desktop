#!/usr/bin/env swift
import AppKit

enum IconFailure: Error {
    case invalidSource, bitmapCreation(Int), encoding(Int), iconutil(Int32)
}

func render(_ image: NSImage, pixels: Int, to url: URL) throws {
    guard let colorSpace = CGColorSpace(name: CGColorSpace.sRGB),
          let bitmap = CGContext(data: nil, width: pixels, height: pixels, bitsPerComponent: 8, bytesPerRow: 0,
                                 space: colorSpace, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
        throw IconFailure.bitmapCreation(pixels)
    }
    NSGraphicsContext.saveGraphicsState()
    let context = NSGraphicsContext(cgContext: bitmap, flipped: false)
    NSGraphicsContext.current = context
    context.imageInterpolation = .high
    image.draw(in: NSRect(x: 0, y: 0, width: pixels, height: pixels))
    NSGraphicsContext.restoreGraphicsState()
    guard let cgImage = bitmap.makeImage(),
          let png = NSBitmapImageRep(cgImage: cgImage).representation(using: .png, properties: [:]) else {
        throw IconFailure.encoding(pixels)
    }
    try png.write(to: url, options: .atomic)
}

do {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let assets = root.appendingPathComponent("apps/macos/Support/Assets", isDirectory: true)
    let iconset = root.appendingPathComponent("artifacts/macos-icon/AppIcon.iconset", isDirectory: true)
    guard let image = NSImage(contentsOf: assets.appendingPathComponent("AppIcon.svg")), image.size == NSSize(width: 1024, height: 1024) else {
        throw IconFailure.invalidSource
    }
    try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
    for points in [16, 32, 128, 256, 512] {
        for scale in [1, 2] {
            let name = "icon_\(points)x\(points)" + (scale == 2 ? "@2x" : "") + ".png"
            try render(image, pixels: points * scale, to: iconset.appendingPathComponent(name))
        }
    }
    try Data(contentsOf: iconset.appendingPathComponent("icon_512x512@2x.png"))
        .write(to: assets.appendingPathComponent("AppIcon.png"), options: .atomic)
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
    process.arguments = ["-c", "icns", iconset.path, "-o", assets.appendingPathComponent("AppIcon.icns").path]
    try process.run()
    process.waitUntilExit()
    guard process.terminationStatus == 0 else { throw IconFailure.iconutil(process.terminationStatus) }
    print("Generated AppIcon.png and AppIcon.icns from AppIcon.svg")
} catch {
    FileHandle.standardError.write(Data("Icon generation failed: \(error)\n".utf8))
    exit(EXIT_FAILURE)
}
