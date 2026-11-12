import AppKit
import Foundation
import JavaScriptCore
import KrillCore
@preconcurrency import WebKit
import XCTest
@testable import KrillMac

final class LoginWindowControllerTests: XCTestCase {
    @MainActor
    func testCommandVPastesIntoSecureSessionFieldAndCommandAReplacesIt() async throws {
        let application = NSApplication.shared
        let previousMenu = application.mainMenu
        application.mainMenu = ApplicationMenu.make()
        let pasteboard = NSPasteboard.general
        let previousContents = (pasteboard.pasteboardItems ?? []).map { item in
            item.types.compactMap { type in item.data(forType: type).map { (type, $0) } }
        }
        defer {
            application.mainMenu = previousMenu
            pasteboard.clearContents()
            let items = previousContents.map { contents in
                let item = NSPasteboardItem()
                for (type, data) in contents { item.setData(data, forType: type) }
                return item
            }
            pasteboard.writeObjects(items)
        }
        let controller = LoginWindowController(store: DashboardStore(storage: try isolatedStorage(), demo: true))
        controller.show()
        defer { controller.close() }
        let window = try XCTUnwrap(application.windows.first { $0.title == "连接 Krill 账号" && $0.isVisible })
        let field = try XCTUnwrap(descendants(window.contentView!).compactMap { $0 as? NSSecureTextField }.first)
        window.makeKeyAndOrderFront(nil)
        XCTAssertTrue(window.makeFirstResponder(field))
        let editor = try XCTUnwrap(field.currentEditor())
        XCTAssertTrue(window.firstResponder === editor)
        // Exercise the login window's real dispatch path. Do not retarget the
        // Edit menu to the editor: that hid the accessory-app routing failure.
        func shortcut(_ key: String, code: UInt16) throws {
            let event = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command,
                timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil,
                characters: key, charactersIgnoringModifiers: key, isARepeat: false, keyCode: code))
            XCTAssertTrue(window.performKeyEquivalent(with: event))
        }
        for token in ["synthetic.header.signature", "replacement.payload.signature"] {
            pasteboard.clearContents(); pasteboard.setString(token, forType: .string)
            try shortcut("a", code: 0)
            try shortcut("v", code: 9)
            XCTAssertEqual(editor.string, token)
        }
        window.makeFirstResponder(nil)
        XCTAssertEqual(field.stringValue, "replacement.payload.signature")
        // The button remains usable when keyboard focus is on another control.
        let paste = try XCTUnwrap(descendants(window.contentView!).compactMap { $0 as? NSButton }.first { $0.title == "粘贴" })
        pasteboard.clearContents(); pasteboard.setString("button.payload.signature", forType: .string)
        paste.performClick(nil)
        XCTAssertEqual(field.stringValue, "button.payload.signature")
        XCTAssertEqual(field.currentEditor()?.string, "button.payload.signature")
        window.makeFirstResponder(nil)
        pasteboard.clearContents()
        paste.performClick(nil)
        XCTAssertEqual(field.stringValue, "button.payload.signature")
    }

    @MainActor
    func testBrowserGuideIsDefaultAndCanReopenWithoutStartingWebLogin() async throws {
        if ProcessInfo.processInfo.environment["KRILL_LOGIN_SCREENSHOT"] != nil {
            NSApplication.shared.setActivationPolicy(.accessory)
        }
        let controller = LoginWindowController(store: DashboardStore(storage: try isolatedStorage(), demo: true))
        defer { controller.close() }
        for _ in 0..<2 {
            controller.show()
            try await Task.sleep(for: .milliseconds(150))
            let window = try XCTUnwrap(NSApp.windows.first { $0.title == "连接 Krill 账号" && $0.isVisible })
            XCTAssertFalse(descendants(window.contentView!).contains { $0 is WKWebView })
            XCTAssertTrue(descendants(window.contentView!).contains { $0 is NSSecureTextField })
            window.contentView?.layoutSubtreeIfNeeded()
            XCTAssertFalse(window.contentView!.hasAmbiguousLayout)
            let title = try XCTUnwrap(descendants(window.contentView!).compactMap { $0 as? NSTextField }.first { $0.stringValue == "连接 Krill 账号" })
            let mode = try XCTUnwrap(descendants(window.contentView!).compactMap { $0 as? NSButton }.first { $0.title == "改用应用内登录" })
            XCTAssertGreaterThanOrEqual(mode.frame.minX, title.frame.maxX + 16)
            if let path = ProcessInfo.processInfo.environment["KRILL_LOGIN_SCREENSHOT"] {
                let capture = Process(); capture.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
                capture.arguments = ["-x", "-l", String(window.windowNumber), path]
                try capture.run(); capture.waitUntilExit(); XCTAssertEqual(capture.terminationStatus, 0)
            }
            controller.close()
            XCTAssertFalse(window.isVisible)
        }
    }

    func testExportScriptOnlyReadsSessionOnExactKrillOrigin() throws {
        let context = try XCTUnwrap(JSContext())
        for (origin, token, expectedReads, expectedPrompts) in [
            ("https://www.krill-code.com", "synthetic.payload.signature", 1, 1),
            ("https://www.krill-code.com", "", 1, 0),
            ("https://www.krill-code.com.evil.example", "synthetic.payload.signature", 0, 0),
            ("https://github.com", "synthetic.payload.signature", 0, 0),
            ("http://www.krill-code.com", "synthetic.payload.signature", 0, 0)
        ] {
            context.setObject(origin, forKeyedSubscript: "testOrigin" as NSString)
            context.setObject(token, forKeyedSubscript: "testToken" as NSString)
            let scriptData = try JSONSerialization.data(withJSONObject: [BrowserLogin.exportScript])
            let encoded = String(decoding: scriptData, as: UTF8.self)
            context.evaluateScript("""
            var reads = 0, prompts = 0, exported = null;
            Function('location', 'localStorage', 'alert', 'prompt', \(encoded)[0])(
              {origin: testOrigin},
              {getItem: key => { if (key !== 'krill_jwt') throw Error('Unexpected storage key'); reads++; return testToken; }},
              () => {}, (message, value) => { prompts++; exported = value; });
            """)
            XCTAssertNil(context.exception)
            XCTAssertEqual(context.objectForKeyedSubscript("reads")?.toInt32(), Int32(expectedReads))
            XCTAssertEqual(context.objectForKeyedSubscript("prompts")?.toInt32(), Int32(expectedPrompts))
            if expectedPrompts == 1 { XCTAssertEqual(context.objectForKeyedSubscript("exported")?.toString(), token) }
        }
    }

    @MainActor
    func testOAuthPopupPreservesOpenerAndClosesIndependently() async throws {
        let controller = LoginWindowController(store: DashboardStore(storage: try isolatedStorage(), demo: true))
        controller.show()
        defer { controller.close() }
        let config = WKWebViewConfiguration(); config.websiteDataStore = .nonPersistent()
        config.preferences.javaScriptCanOpenWindowsAutomatically = true
        let opener = WKWebView(frame: .zero, configuration: config); opener.uiDelegate = controller
        opener.loadHTMLString("<html><body>Popup regression</body></html>", baseURL: nil)
        for _ in 0..<100 {
            if (try? await evaluate("document.body ? document.body.textContent : ''", in: opener)) == "Popup regression" { break }
            try await Task.sleep(for: .milliseconds(50))
        }
        _ = try await evaluate("window.addEventListener('message', e => window.popupResult = e.data); window.child = window.open('about:blank'); 'opened'", in: opener)
        let popup = try XCTUnwrap(NSApp.windows.first { $0.isVisible && $0.contentView is WKWebView })
        let child = try XCTUnwrap(popup.contentView as? WKWebView)
        for _ in 0..<100 {
            if (try? await evaluate("document.readyState", in: child)) == "complete" { break }
            try await Task.sleep(for: .milliseconds(50))
        }
        XCTAssertTrue(child.configuration.websiteDataStore === opener.configuration.websiteDataStore)
        let relationship = try await evaluate("window.opener ? 'has opener' : 'missing'", in: child)
        XCTAssertEqual(relationship, "has opener")
        _ = try await evaluate("window.opener.postMessage('callback received', '*'); 'sent'", in: child)
        _ = try await evaluate("window.child.close(); 'closed'", in: opener)
        for _ in 0..<100 {
            if !popup.isVisible { break }
            try await Task.sleep(for: .milliseconds(50))
        }
        XCTAssertFalse(popup.isVisible)
        let result = try await evaluate("window.popupResult", in: opener)
        XCTAssertEqual(result, "callback received")
        XCTAssertTrue(NSApp.windows.contains { $0.title == "连接 Krill 账号" && $0.isVisible })
        _ = try await evaluate("window.open('about:blank'); 'opened again'", in: opener)
        let second = try XCTUnwrap(NSApp.windows.first { $0.isVisible && $0.contentView is WKWebView })
        controller.close()
        XCTAssertFalse(second.isVisible)
    }

    @MainActor
    private func isolatedStorage() throws -> LocalStorage {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("KrillLogin-\(UUID().uuidString)")
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        return try LocalStorage(demo: true, directory: directory)
    }

    @MainActor
    private func evaluate(_ script: String, in browser: WKWebView) async throws -> String {
        try await withCheckedThrowingContinuation { continuation in
            browser.evaluateJavaScript(script) { value, error in
                if let error {
                    XCTFail("JavaScript failed: \(script): \(error)")
                    continuation.resume(throwing: error)
                }
                else { continuation.resume(returning: value as? String ?? "") }
            }
        }
    }
    @MainActor
    private func descendants(_ view: NSView) -> [NSView] { view.subviews.flatMap { [$0] + descendants($0) } }
    @MainActor
    func testNavigationPolicyCallbacksAreRegistered() async {
        for callback in [
            "webView:decidePolicyForNavigationAction:decisionHandler:",
            "webView:decidePolicyForNavigationResponse:decisionHandler:"
        ] {
            XCTAssertTrue(LoginWindowController.instancesRespond(to: NSSelectorFromString(callback)), callback)
        }
    }
}
