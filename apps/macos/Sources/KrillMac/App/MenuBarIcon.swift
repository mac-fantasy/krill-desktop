import AppKit

@MainActor
enum MenuBarIcon {
    static func image(percent: Double?, needsLogin: Bool, isStale: Bool) -> NSImage {
        let progress = needsLogin ? nil : percent.flatMap { $0.isFinite ? min(100, max(0, $0)) : nil }
        let image = NSImage(size: NSSize(width: 22, height: 22), flipped: false) { rect in
            let center = NSPoint(x: rect.midX, y: rect.midY)
            let ring = NSRect(x: center.x - 9, y: center.y - 9, width: 18, height: 18)
            let track = NSBezierPath(ovalIn: ring)
            track.lineWidth = 1.8
            NSColor.black.withAlphaComponent(0.22).setStroke()
            track.stroke()

            NSColor.black.setStroke()
            if let progress, progress > 0 {
                let arc = NSBezierPath()
                arc.lineWidth = 1.8; arc.lineCapStyle = .round
                if progress >= 100 { arc.appendOval(in: ring) }
                else { arc.appendArc(withCenter: center, radius: 9, startAngle: 90, endAngle: 90 - CGFloat(progress) * 3.6, clockwise: true) }
                arc.stroke()
            }

            drawKrill(at: center)
            if needsLogin || isStale {
                let badge = NSRect(x: center.x + 5, y: center.y - 10, width: 4, height: 4)
                let dot = NSBezierPath(ovalIn: badge)
                // Clear the ring under the badge so the state remains visible at 100%.
                NSGraphicsContext.saveGraphicsState()
                NSGraphicsContext.current?.compositingOperation = .copy
                NSColor.clear.setFill()
                NSBezierPath(ovalIn: badge.insetBy(dx: -0.8, dy: -0.8)).fill()
                NSGraphicsContext.restoreGraphicsState()
                if needsLogin {
                    dot.lineWidth = 1; NSColor.black.setStroke(); dot.stroke()
                } else { NSColor.black.setFill(); dot.fill() }
            }
            return true
        }
        // Template images use their alpha mask, including the translucent track, in either appearance.
        image.isTemplate = true
        return image
    }

    private static func drawKrill(at center: NSPoint) {
        // Keep the app icon's segmented silhouette; a cutout eye survives menu-bar scaling.
        let body = NSBezierPath()
        body.move(to: NSPoint(x: 331, y: 112))
        body.line(to: NSPoint(x: 553, y: 112))
        body.curve(to: NSPoint(x: 331, y: 346), controlPoint1: NSPoint(x: 553, y: 241), controlPoint2: NSPoint(x: 461, y: 346))
        body.close()
        body.move(to: NSPoint(x: 309, y: 112))
        body.curve(to: NSPoint(x: 155, y: 177), controlPoint1: NSPoint(x: 250, y: 119), controlPoint2: NSPoint(x: 194, y: 143))
        body.line(to: NSPoint(x: 309, y: 330)); body.close()
        body.move(to: NSPoint(x: 138, y: 195))
        body.curve(to: NSPoint(x: 72, y: 348), controlPoint1: NSPoint(x: 97, y: 240), controlPoint2: NSPoint(x: 73, y: 294))
        body.line(to: NSPoint(x: 291, y: 348)); body.close()
        body.move(to: NSPoint(x: 72, y: 371))
        body.curve(to: NSPoint(x: 135, y: 532), controlPoint1: NSPoint(x: 72, y: 432), controlPoint2: NSPoint(x: 94, y: 490))
        body.line(to: NSPoint(x: 295, y: 371)); body.close()
        body.move(to: NSPoint(x: 154, y: 550))
        body.curve(to: NSPoint(x: 309, y: 610), controlPoint1: NSPoint(x: 197, y: 586), controlPoint2: NSPoint(x: 251, y: 607))
        body.line(to: NSPoint(x: 309, y: 394)); body.close()
        body.move(to: NSPoint(x: 331, y: 394))
        body.line(to: NSPoint(x: 491, y: 547))
        body.curve(to: NSPoint(x: 331, y: 610), controlPoint1: NSPoint(x: 449, y: 585), controlPoint2: NSPoint(x: 391, y: 608))
        body.close()
        body.appendOval(in: NSRect(x: 387, y: 162, width: 96, height: 96))
        body.windingRule = .evenOdd

        let antennae = NSBezierPath()
        antennae.move(to: NSPoint(x: 550, y: 116))
        antennae.curve(to: NSPoint(x: 482, y: 42), controlPoint1: NSPoint(x: 548, y: 73), controlPoint2: NSPoint(x: 522, y: 42))
        antennae.line(to: NSPoint(x: 132, y: 42))
        antennae.move(to: NSPoint(x: 548, y: 116))
        antennae.curve(to: NSPoint(x: 473, y: 78), controlPoint1: NSPoint(x: 532, y: 91), controlPoint2: NSPoint(x: 505, y: 78))
        antennae.line(to: NSPoint(x: 211, y: 78))

        let transform = AffineTransform(translationByX: center.x - 312 * 0.023, byY: center.y + 326 * 0.023)
        var scaled = transform
        scaled.scale(x: 0.023, y: -0.023)
        body.transform(using: scaled); antennae.transform(using: scaled)
        NSColor.black.setFill(); body.fill()
        antennae.lineWidth = 0.6; antennae.lineCapStyle = .round
        NSColor.black.setStroke(); antennae.stroke()
    }
}
