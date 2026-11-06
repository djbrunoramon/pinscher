// Generates the Pinscher artwork. Run: swift scripts/make-icon.swift
//   packaging/Pinscher.iconset/   -> iconutil -c icns packaging/Pinscher.iconset -o packaging/Pinscher.icns
//   packaging/PinscherTemplate.png and @2x -> the menu bar icon (a template image)
// The app uses the pinscher head; the menu bar uses a full-body profile silhouette.
import AppKit

// MARK: - The pinscher head, in a 100 x 100 box (origin bottom-left)

/// Outer silhouette: two tall pointed ears, the skull, cheeks and a narrow muzzle.
func headOutline() -> NSBezierPath {
    let p = NSBezierPath()
    func pt(_ x: CGFloat, _ y: CGFloat) -> NSPoint { NSPoint(x: x, y: y) }
    p.move(to: pt(15, 99))                                                      // left ear tip
    p.curve(to: pt(19, 66), controlPoint1: pt(12, 88), controlPoint2: pt(15, 76))
    p.curve(to: pt(17, 52), controlPoint1: pt(16, 62), controlPoint2: pt(15, 57)) // temple
    p.curve(to: pt(31, 31), controlPoint1: pt(19, 41), controlPoint2: pt(25, 35)) // cheek
    p.curve(to: pt(35, 10), controlPoint1: pt(32, 24), controlPoint2: pt(33, 16)) // muzzle side
    p.curve(to: pt(50, 4), controlPoint1: pt(38, 6), controlPoint2: pt(44, 4))
    p.curve(to: pt(65, 10), controlPoint1: pt(56, 4), controlPoint2: pt(62, 6))
    p.curve(to: pt(69, 31), controlPoint1: pt(67, 16), controlPoint2: pt(68, 24))
    p.curve(to: pt(83, 52), controlPoint1: pt(75, 35), controlPoint2: pt(81, 41))
    p.curve(to: pt(81, 66), controlPoint1: pt(85, 57), controlPoint2: pt(84, 62))
    p.curve(to: pt(85, 99), controlPoint1: pt(85, 76), controlPoint2: pt(88, 88))   // right ear tip
    p.curve(to: pt(67, 75), controlPoint1: pt(79, 90), controlPoint2: pt(73, 82))
    p.curve(to: pt(33, 75), controlPoint1: pt(58, 72), controlPoint2: pt(42, 72))  // skull top
    p.curve(to: pt(15, 99), controlPoint1: pt(27, 82), controlPoint2: pt(21, 90))
    p.close()
    return p
}

func eye(center: NSPoint, tilt: CGFloat) -> NSBezierPath {
    let r = NSBezierPath(ovalIn: NSRect(x: -5, y: -2.5, width: 10, height: 5))
    var t = AffineTransform(translationByX: center.x, byY: center.y)
    t.rotate(byDegrees: tilt)
    r.transform(using: t)
    return r
}

func nose() -> NSBezierPath {
    NSBezierPath(roundedRect: NSRect(x: 41, y: 7, width: 18, height: 10), xRadius: 5, yRadius: 5)
}

/// The tan markings of the breed: eyebrow dots and cheek patches.
func tanMarkings() -> [NSBezierPath] {
    func quad(_ pts: [(CGFloat, CGFloat)]) -> NSBezierPath {
        let p = NSBezierPath()
        p.move(to: NSPoint(x: pts[0].0, y: pts[0].1))
        for q in pts.dropFirst() { p.line(to: NSPoint(x: q.0, y: q.1)) }
        p.close()
        return p
    }
    return [NSBezierPath(ovalIn: NSRect(x: 27, y: 61, width: 8, height: 5)),
            NSBezierPath(ovalIn: NSRect(x: 65, y: 61, width: 8, height: 5)),
            quad([(21, 44), (32, 29), (35, 14), (42, 18), (42, 35), (34, 45)]),
            quad([(79, 44), (68, 29), (65, 14), (58, 18), (58, 35), (66, 45)])]
}

func transform(_ path: NSBezierPath, into rect: NSRect) -> NSBezierPath {
    let copy = path.copy() as! NSBezierPath
    var t = AffineTransform(translationByX: rect.minX, byY: rect.minY)
    t.scale(x: rect.width / 100, y: rect.height / 100)
    copy.transform(using: t)
    return copy
}

// MARK: - Menu bar template (black on transparent)

func template(points: CGFloat, scale: CGFloat) -> NSImage {
    let px = points * scale
    return NSImage(size: NSSize(width: px, height: px), flipped: false) { rect in
        // Original left-facing pinscher in a 120 x 120 box, origin bottom-left.
        let p = NSBezierPath()
        func pt(_ x: CGFloat, _ y: CGFloat) -> NSPoint { NSPoint(x: x, y: y) }

        // Two narrow upright ears above a small, rounded skull.
        p.move(to: pt(29, 113))
        p.curve(to: pt(36, 99), controlPoint1: pt(32, 110), controlPoint2: pt(34, 103))
        p.line(to: pt(41, 99))
        p.line(to: pt(46, 115))
        p.curve(to: pt(50, 97), controlPoint1: pt(50, 108), controlPoint2: pt(50, 102))
        p.curve(to: pt(49, 87), controlPoint1: pt(52, 94), controlPoint2: pt(51, 90))

        // Long erect neck, a short back sloping gently towards the rump.
        p.curve(to: pt(57, 68), controlPoint1: pt(49, 79), controlPoint2: pt(51, 71))
        p.curve(to: pt(86, 63), controlPoint1: pt(64, 65), controlPoint2: pt(78, 65))

        // Short raised tail, with a substantial stem at 18 pt.
        p.curve(to: pt(97, 73), controlPoint1: pt(92, 65), controlPoint2: pt(95, 69))
        p.curve(to: pt(102, 74), controlPoint1: pt(99, 77), controlPoint2: pt(102, 77))
        p.curve(to: pt(96, 60), controlPoint1: pt(102, 68), controlPoint2: pt(100, 64))

        // Near hind leg: rounded thigh, backward hock and a small level paw.
        p.curve(to: pt(96, 40), controlPoint1: pt(99, 55), controlPoint2: pt(95, 46))
        p.curve(to: pt(103, 23), controlPoint1: pt(96, 34), controlPoint2: pt(100, 27))
        p.line(to: pt(105, 7))
        p.line(to: pt(96, 7))
        p.curve(to: pt(96, 12), controlPoint1: pt(94, 7), controlPoint2: pt(94, 11))
        p.line(to: pt(95, 23))
        p.curve(to: pt(83, 43), controlPoint1: pt(89, 28), controlPoint2: pt(85, 36))

        // Far hind leg, separated from the nearer hock by open space.
        p.curve(to: pt(83, 28), controlPoint1: pt(83, 36), controlPoint2: pt(84, 32))
        p.line(to: pt(81, 7))
        p.line(to: pt(72, 7))
        p.curve(to: pt(74, 12), controlPoint1: pt(70, 7), controlPoint2: pt(72, 11))
        p.line(to: pt(76, 27))
        p.curve(to: pt(72, 40), controlPoint1: pt(72, 31), controlPoint2: pt(71, 36))
        p.curve(to: pt(75, 54), controlPoint1: pt(72, 45), controlPoint2: pt(74, 49))

        // A short continuous belly arc: tucked waist to deep chest, no wedge.
        p.curve(to: pt(55, 45), controlPoint1: pt(66, 54), controlPoint2: pt(64, 45))
        p.curve(to: pt(53, 40), controlPoint1: pt(53, 45), controlPoint2: pt(53, 43))
        p.line(to: pt(53, 7))
        p.line(to: pt(44, 7))
        p.curve(to: pt(46, 12), controlPoint1: pt(42, 7), controlPoint2: pt(43, 11))
        p.line(to: pt(46, 43))
        p.line(to: pt(40, 43))
        p.line(to: pt(40, 7))
        p.line(to: pt(30, 7))
        p.curve(to: pt(33, 12), controlPoint1: pt(28, 7), controlPoint2: pt(30, 11))
        p.line(to: pt(33, 40))

        // Projecting chest flows into the throat without an angular elbow.
        p.curve(to: pt(31, 57), controlPoint1: pt(33, 45), controlPoint2: pt(28, 49))
        p.curve(to: pt(35, 79), controlPoint1: pt(34, 65), controlPoint2: pt(35, 72))
        p.curve(to: pt(26, 79), controlPoint1: pt(32, 78), controlPoint2: pt(29, 78))

        // Medium muzzle, round nose and a visible stop below the forehead.
        p.line(to: pt(17, 82))
        p.curve(to: pt(17, 88), controlPoint1: pt(13, 83), controlPoint2: pt(13, 87))
        p.line(to: pt(26, 89))
        p.curve(to: pt(29, 98), controlPoint1: pt(26, 93), controlPoint2: pt(27, 96))
        p.curve(to: pt(29, 113), controlPoint1: pt(27, 103), controlPoint2: pt(28, 109))
        p.close()

        // Preserve the silhouette and edge margins; no subpixel facial cutouts.
        let k = min(rect.width, rect.height) / 120
        var t = AffineTransform(translationByX: rect.midX - 60 * k, byY: rect.midY - 61 * k)
        t.scale(k)
        p.transform(using: t)
        NSColor.black.setFill()
        p.fill()
        return true
    }
}

// MARK: - App icon

func appIcon(size: CGFloat) -> NSImage {
    NSImage(size: NSSize(width: size, height: size), flipped: false) { rect in
        let s = size / 1024
        let body = rect.insetBy(dx: 100 * s, dy: 100 * s)
        let squircle = NSBezierPath(roundedRect: body, xRadius: 185 * s, yRadius: 185 * s)

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
        NSGradient(colors: [NSColor(red: 0.98, green: 0.74, blue: 0.36, alpha: 1),
                            NSColor(red: 0.96, green: 0.45, blue: 0.30, alpha: 1),
                            NSColor(red: 0.62, green: 0.22, blue: 0.62, alpha: 1)],
                   atLocations: [0, 0.5, 1], colorSpace: .deviceRGB)!.draw(in: body, angle: -65)
        NSGradient(colors: [NSColor.white.withAlphaComponent(0.22), NSColor.white.withAlphaComponent(0)])!
            .draw(in: NSRect(x: body.minX, y: body.midY, width: body.width, height: body.height / 2), angle: -90)

        // The dog's head, big and centred.
        let side = body.width * 0.80
        let head = NSRect(x: body.midX - side / 2, y: body.minY + body.height * 0.06, width: side, height: side)
        NSGraphicsContext.saveGraphicsState()
        let drop = NSShadow()
        drop.shadowColor = NSColor.black.withAlphaComponent(0.4)
        drop.shadowBlurRadius = 30 * s
        drop.shadowOffset = NSSize(width: 0, height: -14 * s)
        drop.set()
        NSColor(red: 0.09, green: 0.08, blue: 0.11, alpha: 1).setFill()
        transform(headOutline(), into: head).fill()
        NSGraphicsContext.restoreGraphicsState()

        // Soft highlight on the skull.
        NSGraphicsContext.saveGraphicsState()
        transform(headOutline(), into: head).addClip()
        NSGradient(colors: [NSColor.white.withAlphaComponent(0.20), NSColor.white.withAlphaComponent(0)])!
            .draw(in: NSRect(x: head.minX, y: head.minY + head.height * 0.5, width: head.width, height: head.height * 0.5), angle: -90)
        NSGraphicsContext.restoreGraphicsState()

        NSColor(red: 0.88, green: 0.52, blue: 0.20, alpha: 1).setFill()
        for mark in tanMarkings() { transform(mark, into: head).fill() }
        NSColor(red: 1.0, green: 0.86, blue: 0.55, alpha: 1).setFill()
        transform(eye(center: NSPoint(x: 36, y: 54), tilt: -12), into: head).fill()
        transform(eye(center: NSPoint(x: 64, y: 54), tilt: 12), into: head).fill()
        NSColor(red: 0.03, green: 0.03, blue: 0.05, alpha: 1).setFill()
        let n = transform(nose(), into: head); n.fill()
        NSColor.white.withAlphaComponent(0.35).setFill()
        transform(NSBezierPath(ovalIn: NSRect(x: 45, y: 12, width: 7, height: 3)), into: head).fill()
        NSGraphicsContext.restoreGraphicsState()

        NSColor.white.withAlphaComponent(0.3).setStroke()
        squircle.lineWidth = 3 * s
        squircle.stroke()
        return true
    }
}

// MARK: - Output

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

let root = URL(fileURLWithPath: CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "packaging")
let iconset = root.appendingPathComponent("Pinscher.iconset")
try? FileManager.default.removeItem(at: iconset)
try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
let master = appIcon(size: 1024)
for (name, px) in [("16x16", 16), ("16x16@2x", 32), ("32x32", 32), ("32x32@2x", 64), ("128x128", 128),
                   ("128x128@2x", 256), ("256x256", 256), ("256x256@2x", 512), ("512x512", 512),
                   ("512x512@2x", 1024)] {
    try png(master, pixels: px).write(to: iconset.appendingPathComponent("icon_\(name).png"))
}
// The menu bar icon: 18 pt, named ...Template so AppKit treats it as a template image.
try png(template(points: 18, scale: 1), pixels: 18).write(to: root.appendingPathComponent("PinscherTemplate.png"))
try png(template(points: 18, scale: 2), pixels: 36).write(to: root.appendingPathComponent("PinscherTemplate@2x.png"))
print("wrote \(root.path)")
