import AppKit
import XCTest
@testable import KrillMac

final class PanelFocusTests: XCTestCase {
    @MainActor
    func testOpeningClearsControlFocusAndAllowsLaterKeyboardFocus() async {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 200, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        let button = FocusableButton(frame: NSRect(x: 0, y: 0, width: 100, height: 30))
        window.contentView?.addSubview(button)
        XCTAssertTrue(window.makeFirstResponder(button))
        MenuBarController.clearInitialFocus(in: window)
        XCTAssertFalse(window.firstResponder === button)
        XCTAssertTrue(window.makeFirstResponder(button))
        XCTAssertTrue(window.firstResponder === button)
    }
}

@MainActor
private final class FocusableButton: NSButton { override var acceptsFirstResponder: Bool { true } }
