// Generates packaging/Pinscher.icns. Run: swift scripts/make-icon.swift
// Draws a macOS-style rounded icon: a deep gradient, a translucent menu bar with one dot
// slashed out (the hidden icon) and a white dog peeking over it.
import AppKit

func draw(size: CGFloat) -> NSImage {
    NSImage(size: NSSize(width: size, height: size), flipped: false) { rect in
        let s = size / 1024
        let inset = 100 * s
        let body = rect.insetBy(dx: inset, dy: inset)
        let squircle = NSBezierPath(roundedRect: body, xRadius: 185 * s, yRadius: 185 * s)

        // Soft drop shadow under the tile.
        NSGraphicsContext.saveGraphicsState()
        let shadow = NSShadow()
        shadow.shadowColor = NSColor.black.withAlphaComponent(0.35)
        shadow.shadowBlurRadius = 24 * s
        shadow.shadowOffset = NSSize(width: 0, height: -12 * s)
        shadow.set()
        NSColor.black.setFill()
        squircle.fill()
        NSGraphicsContext.restoreGraphicsState()

        NSGraphicsContext.saveGraphicsState()
        squircle.addClip()
        NSGradient(colors: [
            NSColor(red: 0.07, green: 0.08, blue: 0.20, alpha: 1),
            NSColor(red: 0.22, green: 0.16, blue: 0.62, alpha: 1),
            NSColor(red: 0.46, green: 0.27, blue: 0.95, alpha: 1),
        ], atLocations: [0, 0.55, 1], colorSpace: .deviceRGB)!.draw(in: body, angle: -60)

        // Light from the top-left.
        NSGradient(colors: [NSColor.white.withAlphaComponent(0.10), NSColor.white.withAlphaComponent(0)])!
            .draw(in: NSRect(x: body.minX, y: body.midY, width: body.width, height: body.height / 2), angle: -90)

        // The menu bar strip.
        let bar = NSRect(x: body.minX + 70 * s, y: body.maxY - 205 * s, width: body.width - 140 * s, height: 96 * s)
        let barPath = NSBezierPath(roundedRect: bar, xRadius: 48 * s, yRadius: 48 * s)
        NSColor.white.withAlphaComponent(0.20).setFill()
        barPath.fill()
        NSColor.white.withAlphaComponent(0.35).setStroke()
        barPath.lineWidth = 3 * s
        barPath.stroke()
        let dot = 38 * s
        for i in 0..<4 {
            let x = bar.minX + 44 * s + CGFloat(i) * 98 * s
            let r = NSRect(x: x, y: bar.midY - dot / 2, width: dot, height: dot)
            let p = NSBezierPath(ovalIn: r)
            if i == 3 {
                // The hidden icon: hollow and slashed.
                NSColor.white.withAlphaComponent(0.55).setStroke()
                p.lineWidth = 5 * s
                p.stroke()
                let slash = NSBezierPath()
                slash.move(to: NSPoint(x: r.minX - 4 * s, y: r.minY - 4 * s))
                slash.line(to: NSPoint(x: r.maxX + 4 * s, y: r.maxY + 4 * s))
                slash.lineWidth = 6 * s
                slash.lineCapStyle = .round
                slash.stroke()
            } else {
                NSColor.white.withAlphaComponent(0.9).setFill()
                p.fill()
            }
        }

        // The dog, in white, with a soft glow.
        if let symbol = NSImage(systemSymbolName: "dog.fill", accessibilityDescription: nil) {
            let config = NSImage.SymbolConfiguration(pointSize: 400 * s, weight: .bold)
            let big = symbol.withSymbolConfiguration(config) ?? symbol
            let tinted = NSImage(size: big.size, flipped: false) { r in
                big.draw(in: r)
                NSColor.white.set()
                r.fill(using: .sourceIn)
                return true
            }
            let w = tinted.size.width, h = tinted.size.height
            let target = NSRect(x: body.midX - w / 2, y: body.minY + 95 * s, width: w, height: h)
            NSGraphicsContext.saveGraphicsState()
            let glow = NSShadow()
            glow.shadowColor = NSColor(red: 0.6, green: 0.5, blue: 1, alpha: 0.9)
            glow.shadowBlurRadius = 40 * s
            glow.set()
            tinted.draw(in: target)
            NSGraphicsContext.restoreGraphicsState()
        }
        NSGraphicsContext.restoreGraphicsState()

        // Thin inner highlight on the edge.
        NSColor.white.withAlphaComponent(0.25).setStroke()
        squircle.lineWidth = 3 * s
        squircle.stroke()
        return true
    }
}

func png(_ image: NSImage, pixels: Int) -> Data {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8,
                               samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                               bytesPerRow: 0, bitsPerPixel: 0)!
    rep.size = NSSize(width: pixels, height: pixels)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    image.draw(in: NSRect(x: 0, y: 0, width: pixels, height: pixels))
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}

let out = URL(fileURLWithPath: CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "packaging/Pinscher.iconset")
try? FileManager.default.removeItem(at: out)
try FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
let master = draw(size: 1024)
for (name, px) in [("16x16", 16), ("16x16@2x", 32), ("32x32", 32), ("32x32@2x", 64), ("128x128", 128),
                   ("128x128@2x", 256), ("256x256", 256), ("256x256@2x", 512), ("512x512", 512),
                   ("512x512@2x", 1024)] {
    try png(master, pixels: px).write(to: out.appendingPathComponent("icon_\(name).png"))
}
print("wrote \(out.path)")
