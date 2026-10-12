// Draws Resources/AppIcon.icns: a disc split between day and night.
// Run with: swift tools/make-icon.swift
import AppKit

func render(_ size: CGFloat) -> NSBitmapImageRep {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(size), pixelsHigh: Int(size),
                               bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                               colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let s = size / 1024

    // Squircle background, warm sky on the left fading into night on the right.
    let inset = 100 * s
    let tile = NSRect(x: inset, y: inset, width: size - 2 * inset, height: size - 2 * inset)
    let shape = NSBezierPath(roundedRect: tile, xRadius: 185 * s, yRadius: 185 * s)
    NSGradient(colors: [NSColor(red: 1.00, green: 0.80, blue: 0.45, alpha: 1),
                        NSColor(red: 0.95, green: 0.45, blue: 0.40, alpha: 1),
                        NSColor(red: 0.20, green: 0.17, blue: 0.42, alpha: 1),
                        NSColor(red: 0.07, green: 0.07, blue: 0.20, alpha: 1)],
               atLocations: [0, 0.42, 0.58, 1], colorSpace: .deviceRGB)!
        .draw(in: shape, angle: -35)

    // The disc: sun half, then the night half clipped over it.
    let r = 250 * s, c = NSPoint(x: size / 2, y: size / 2)
    let disc = NSBezierPath(ovalIn: NSRect(x: c.x - r, y: c.y - r, width: 2 * r, height: 2 * r))
    NSColor(red: 1.0, green: 0.97, blue: 0.88, alpha: 1).setFill()
    disc.fill()
    NSGraphicsContext.saveGraphicsState()
    disc.addClip()
    let night = NSBezierPath()
    night.move(to: NSPoint(x: c.x + r * 2, y: c.y + r * 2))
    night.line(to: NSPoint(x: c.x - r * 2, y: c.y + r * 2))
    night.line(to: NSPoint(x: c.x + r * 2, y: c.y - r * 2))
    night.close()
    let tilt = NSAffineTransform()
    tilt.translateX(by: c.x, yBy: c.y); tilt.rotate(byDegrees: -45); tilt.translateX(by: -c.x, yBy: -c.y)
    let halfPlane = NSBezierPath(rect: NSRect(x: c.x, y: c.y - 2 * r, width: 2 * r, height: 4 * r))
    halfPlane.transform(using: tilt as AffineTransform)
    NSColor(red: 0.13, green: 0.12, blue: 0.30, alpha: 1).setFill()
    halfPlane.fill()
    NSGraphicsContext.restoreGraphicsState()

    NSGraphicsContext.restoreGraphicsState()
    return rep
}

let fm = FileManager.default
let iconset = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("AppIcon.iconset")
try? fm.removeItem(at: iconset)
try! fm.createDirectory(at: iconset, withIntermediateDirectories: true)
for base in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let name = scale == 1 ? "icon_\(base)x\(base).png" : "icon_\(base)x\(base)@2x.png"
        let png = render(CGFloat(base * scale)).representation(using: .png, properties: [:])!
        try! png.write(to: iconset.appendingPathComponent(name))
    }
}
let task = Process()
task.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
task.arguments = ["-c", "icns", iconset.path, "-o", "Resources/AppIcon.icns"]
try! task.run(); task.waitUntilExit()
print(task.terminationStatus == 0 ? "✓ Resources/AppIcon.icns" : "iconutil a échoué")
