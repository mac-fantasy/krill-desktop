import AppKit
import KrillCore
@preconcurrency import WebKit

@MainActor
final class LoginWindowController: NSObject, NSWindowDelegate, WKNavigationDelegate, WKUIDelegate {
    private let store: DashboardStore
    private var window: NSWindow?
    private var webView: WKWebView?
    private var popups: [WKWebView: NSWindow] = [:]
    private let status = NSTextField(wrappingLabelWithString: "选择浏览器登录，可使用已有登录状态及官网提供的任意登录方式。")
    private let sessionField = NSSecureTextField()
    private let browserGuide = NSStackView()
    private let embeddedContainer = NSView()
    private let location = NSTextField(labelWithString: "")
    private let modeButton = NSButton(title: "改用应用内登录", target: nil, action: nil)
    private var embedded = false
    private var pollingTask: Task<Void, Never>?
    private var validationTask: Task<Void, Never>?
    private var checking = false
    private var rejectedToken: String?
    var didClose: (() -> Void)?

    init(store: DashboardStore) { self.store = store }

    func show() {
        if let window { NSApp.activate(ignoringOtherApps: true); window.makeKeyAndOrderFront(nil); return }
        let title = NSTextField(labelWithString: "连接 Krill 账号")
        title.font = .systemFont(ofSize: 20, weight: .semibold)
        let intro = NSTextField(wrappingLabelWithString: "在默认浏览器中登录，支持 GitHub、Apple 二次登录及官网其他登录方式。")
        let open = NSButton(title: "在浏览器中登录 ↗", target: self, action: #selector(openBrowser))
        open.bezelStyle = .rounded
        let steps = NSTextField(wrappingLabelWithString: "1. 在浏览器中完成 Krill 登录；已登录可直接使用。\n2. 在 Krill 官网打开开发者工具的“控制台”，粘贴并运行下方按钮复制的脚本。\n3. 复制弹出的会话，粘贴到下面并连接。\n\n官网目前不支持自动回传桌面端，因此需要手动导入一次。会话等同于登录凭据，请勿分享。")
        steps.font = .systemFont(ofSize: 13)
        let copy = NSButton(title: "复制会话导出脚本", target: self, action: #selector(copyExportScript))
        copy.bezelStyle = .rounded
        sessionField.placeholderString = "粘贴 Krill 会话"
        sessionField.target = self; sessionField.action = #selector(importSession)
        let paste = NSButton(title: "粘贴", target: self, action: #selector(pasteSession))
        paste.bezelStyle = .rounded
        let connect = NSButton(title: "验证并连接", target: self, action: #selector(importSession))
        connect.bezelStyle = .rounded
        let input = NSStackView(views: [sessionField, paste, connect]); input.spacing = 10
        sessionField.widthAnchor.constraint(greaterThanOrEqualToConstant: 280).isActive = true
        browserGuide.setViews([intro, open, steps, copy, input], in: .leading)
        browserGuide.orientation = .vertical; browserGuide.alignment = .leading; browserGuide.spacing = 20
        modeButton.target = self; modeButton.action = #selector(toggleMode); modeButton.bezelStyle = .rounded
        modeButton.setContentHuggingPriority(.required, for: .horizontal)
        status.font = .systemFont(ofSize: 12); status.textColor = .secondaryLabelColor
        location.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        location.lineBreakMode = .byTruncatingMiddle
        embeddedContainer.isHidden = true
        let content = NSView()
        for view in [title, modeButton, browserGuide, embeddedContainer, status, location] {
            view.translatesAutoresizingMaskIntoConstraints = false; content.addSubview(view)
        }
        NSLayoutConstraint.activate([
            title.topAnchor.constraint(equalTo: content.topAnchor, constant: 20),
            title.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 20),
            title.trailingAnchor.constraint(lessThanOrEqualTo: modeButton.leadingAnchor, constant: -16),
            modeButton.centerYAnchor.constraint(equalTo: title.centerYAnchor),
            modeButton.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -20),
            browserGuide.topAnchor.constraint(equalTo: title.bottomAnchor, constant: 24),
            browserGuide.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 24),
            browserGuide.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -24),
            browserGuide.bottomAnchor.constraint(lessThanOrEqualTo: status.topAnchor, constant: -20),
            embeddedContainer.topAnchor.constraint(equalTo: title.bottomAnchor, constant: 36),
            embeddedContainer.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            embeddedContainer.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            embeddedContainer.bottomAnchor.constraint(equalTo: status.topAnchor, constant: -12),
            location.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 20),
            location.trailingAnchor.constraint(equalTo: modeButton.leadingAnchor, constant: -12),
            location.topAnchor.constraint(equalTo: title.bottomAnchor, constant: 10),
            status.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 20),
            status.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -20),
            status.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -16)
        ])
        let window = SessionLoginWindow(contentRect: NSRect(x: 0, y: 0, width: 900, height: 720),
                              styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        window.sessionField = sessionField
        window.title = "连接 Krill 账号"; window.minSize = NSSize(width: 700, height: 620)
        window.contentView = content; window.delegate = self; window.isReleasedWhenClosed = false; window.center()
        self.window = window; rejectedToken = nil
        embedded = false; browserGuide.isHidden = false; modeButton.title = "改用应用内登录"
        status.stringValue = "选择浏览器登录，可使用已有登录状态及官网提供的任意登录方式。"
        window.makeKeyAndOrderFront(nil); NSApp.activate(ignoringOtherApps: true)
        pollingTask = Task { [weak self] in
            while !Task.isCancelled {
                do { try await Task.sleep(for: .milliseconds(1500)) } catch { return }
                self?.checkSessions()
            }
        }
    }

    @objc private func openBrowser() {
        if !NSWorkspace.shared.open(BrowserLogin.loginURL) { status.stringValue = "无法打开默认浏览器，请手动访问 https://www.krill-code.com/login。" }
    }
    @objc private func copyExportScript() {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(BrowserLogin.exportScript, forType: .string)
        status.stringValue = "脚本已复制，请仅在 Krill 官网的控制台运行，然后导入弹出的会话。"
    }
    @objc private func importSession() {
        guard !checking else { return }
        guard let token = BrowserLogin.session(from: sessionField.stringValue) else {
            status.stringValue = "会话格式不正确，请粘贴脚本弹出的完整 Krill 会话。"; return
        }
        sessionField.stringValue = ""
        validate(token)
    }
    @objc private func pasteSession() {
        guard !checking else { return }
        guard let value = NSPasteboard.general.string(forType: .string), !value.isEmpty else {
            status.stringValue = "剪贴板中没有文本，请先复制完整 Krill 会话。"; return
        }
        window?.makeFirstResponder(nil)
        sessionField.stringValue = value
        window?.makeFirstResponder(sessionField)
        sessionField.currentEditor()?.selectAll(nil)
        status.stringValue = "会话已粘贴，请点击“验证并连接”。"
    }
    @objc private func toggleMode() {
        guard !checking else { return }
        embedded.toggle(); browserGuide.isHidden = embedded; embeddedContainer.isHidden = !embedded
        modeButton.title = embedded ? "改用浏览器登录" : "改用应用内登录"
        location.isHidden = !embedded
        if !embedded { for popup in Array(popups.values) { popup.close() } }
        status.stringValue = embedded ? "请选择官网任意登录方式；若服务商限制应用内登录，可改用浏览器登录。" : "在默认浏览器登录后导入会话，即可复用已有登录状态。"
        if embedded, webView == nil {
            let config = WKWebViewConfiguration()
            config.websiteDataStore = .nonPersistent()
            config.preferences.javaScriptCanOpenWindowsAutomatically = true
            let browser = makeWebView(configuration: config)
            browser.translatesAutoresizingMaskIntoConstraints = false; embeddedContainer.addSubview(browser)
            NSLayoutConstraint.activate([
                browser.topAnchor.constraint(equalTo: embeddedContainer.topAnchor), browser.bottomAnchor.constraint(equalTo: embeddedContainer.bottomAnchor),
                browser.leadingAnchor.constraint(equalTo: embeddedContainer.leadingAnchor), browser.trailingAnchor.constraint(equalTo: embeddedContainer.trailingAnchor)
            ])
            webView = browser
            browser.load(URLRequest(url: BrowserLogin.loginURL))
        }
    }
    private func makeWebView(configuration: WKWebViewConfiguration) -> WKWebView {
        let browser = WKWebView(frame: .zero, configuration: configuration)
        browser.navigationDelegate = self; browser.uiDelegate = self
        browser.allowsBackForwardNavigationGestures = true; browser.isInspectable = false
        return browser
    }
    func close() { window?.close() }
    func windowWillClose(_ notification: Notification) {
        guard let closing = notification.object as? NSWindow else { return }
        if closing !== window {
            if let browser = popups.first(where: { $0.value === closing })?.key {
                dispose(browser); popups.removeValue(forKey: browser)
            }
            return
        }
        pollingTask?.cancel(); validationTask?.cancel(); checking = false
        let children = popups; popups.removeAll()
        for (browser, popup) in children { dispose(browser); popup.close() }
        if let webView { dispose(webView); webView.removeFromSuperview() }
        sessionField.stringValue = ""; location.stringValue = ""
        webView = nil; window = nil; didClose?()
    }
    private func dispose(_ browser: WKWebView) {
        browser.stopLoading(); browser.navigationDelegate = nil; browser.uiDelegate = nil
    }
    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction,
                 decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void) {
        if navigationAction.targetFrame?.isMainFrame == false { decisionHandler(.allow); return }
        // OAuth popups often open about:blank and navigate it after an asynchronous request.
        let blankPopup = navigationAction.request.url?.absoluteString == "about:blank" && (popups[webView] != nil || navigationAction.targetFrame == nil)
        guard blankPopup || KrillOrigin.allowsLoginNavigation(navigationAction.request.url) else {
            status.stringValue = "此跳转不是有效的 HTTPS 页面；可改用浏览器完成登录。"
            decisionHandler(.cancel); return
        }
        decisionHandler(.allow)
    }
    func webView(_ webView: WKWebView, decidePolicyFor navigationResponse: WKNavigationResponse,
                 decisionHandler: @escaping @MainActor @Sendable (WKNavigationResponsePolicy) -> Void) {
        decisionHandler(navigationResponse.canShowMIMEType ? .allow : .cancel)
    }
    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
                 for navigationAction: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        guard window != nil, popups.count < 5,
              navigationAction.request.url?.absoluteString == "about:blank" || KrillOrigin.allowsLoginNavigation(navigationAction.request.url) else { return nil }
        // Return a real child using WebKit's supplied configuration to preserve opener and shared cookies.
        let browser = makeWebView(configuration: configuration)
        let popup = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 720, height: 680),
                             styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
        popup.title = "Krill · 第三方登录"; popup.minSize = NSSize(width: 500, height: 480)
        popup.contentView = browser; popup.delegate = self; popup.isReleasedWhenClosed = false
        popups[browser] = popup; popup.center(); popup.makeKeyAndOrderFront(nil)
        return browser
    }
    func webViewDidClose(_ webView: WKWebView) { popups[webView]?.close() }
    func webView(_ webView: WKWebView, runJavaScriptAlertPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo,
                 completionHandler: @escaping @MainActor () -> Void) {
        guard let host = webView.window else { completionHandler(); return }
        let alert = scriptAlert(message, frame: frame); alert.addButton(withTitle: "好")
        alert.beginSheetModal(for: host) { _ in completionHandler() }
    }
    func webView(_ webView: WKWebView, runJavaScriptConfirmPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo,
                 completionHandler: @escaping @MainActor (Bool) -> Void) {
        guard let host = webView.window else { completionHandler(false); return }
        let alert = scriptAlert(message, frame: frame); alert.addButton(withTitle: "继续"); alert.addButton(withTitle: "取消")
        alert.beginSheetModal(for: host) { response in completionHandler(response == .alertFirstButtonReturn) }
    }
    func webView(_ webView: WKWebView, runJavaScriptTextInputPanelWithPrompt prompt: String, defaultText: String?, initiatedByFrame frame: WKFrameInfo,
                 completionHandler: @escaping @MainActor (String?) -> Void) {
        guard let host = webView.window else { completionHandler(nil); return }
        let alert = scriptAlert(prompt, frame: frame); alert.addButton(withTitle: "继续"); alert.addButton(withTitle: "取消")
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 300, height: 24)); field.stringValue = defaultText ?? ""
        alert.accessoryView = field
        alert.beginSheetModal(for: host) { response in completionHandler(response == .alertFirstButtonReturn ? field.stringValue : nil) }
    }
    private func scriptAlert(_ message: String, frame: WKFrameInfo) -> NSAlert {
        let alert = NSAlert(); alert.messageText = frame.securityOrigin.host; alert.informativeText = message
        return alert
    }
    func webView(_ webView: WKWebView, didCommit navigation: WKNavigation!) {
        let origin = webView.url.map { "\($0.scheme ?? "https")://\($0.host ?? "")" } ?? ""
        if let popup = popups[webView] { popup.title = "Krill 登录 · " + origin }
        else { location.stringValue = origin }
    }
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { checkSessions() }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        if embedded, (error as NSError).code != NSURLErrorCancelled { status.stringValue = "页面加载失败，可关闭后重试，或改用浏览器登录。" }
    }
    private func checkSessions() {
        guard embedded else { return }
        for browser in ([webView].compactMap { $0 } + Array(popups.keys)) { checkSession(in: browser) }
    }
    private func contains(_ browser: WKWebView) -> Bool { browser === webView || popups[browser] != nil }
    private func checkSession(in browser: WKWebView) {
        guard !checking, let requestWindow = window, contains(browser), KrillOrigin.isTrusted(browser.url) else { return }
        checking = true
        let script = """
        (() => {
          if (location.origin !== 'https://www.krill-code.com') return null;
          return {origin: location.origin, token: localStorage.getItem('krill_jwt')};
        })()
        """
        browser.evaluateJavaScript(script) { [weak self, weak browser] result, _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                guard self.window === requestWindow else { return }
                guard self.embedded, let browser, self.contains(browser), KrillOrigin.isTrusted(browser.url),
                      let object = result as? [String: Any], object["origin"] as? String == "https://www.krill-code.com",
                      let token = object["token"] as? String, KrillOrigin.isPlausibleSession(token), token != self.rejectedToken else {
                    self.checking = false; return
                }
                self.validate(token)
            }
        }
    }
    private func validate(_ token: String) {
        guard let requestWindow = window else { return }
        checking = true; status.stringValue = "正在确认账号…"
        validationTask = Task { [weak self] in
            guard let self else { return }
            defer { if self.window === requestWindow { self.checking = false } }
            do {
                try await self.store.acceptSession(token)
                try Task.checkCancellation()
                if self.window === requestWindow { self.close() }
            } catch is CancellationError { }
            catch let error as APIFailure {
                guard self.window === requestWindow else { return }
                if error == .sessionExpired { self.rejectedToken = token }
                self.status.stringValue = error.message
            } catch {
                if self.window === requestWindow { self.status.stringValue = "暂时无法确认登录或保存钥匙串，请稍后重试。" }
            }
        }
    }
}
