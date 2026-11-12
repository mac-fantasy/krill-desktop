import AppKit

@main
enum KrillApp {
    @MainActor
    static func main() {
        let application = NSApplication.shared
        let delegate = AppDelegate()
        application.delegate = delegate
        application.setActivationPolicy(.accessory)
        application.mainMenu = ApplicationMenu.make()
        // AppKit holds its delegate weakly; retain it throughout the event loop.
        withExtendedLifetime(delegate) { application.run() }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var store: DashboardStore?
    private var menuBar: MenuBarController?
    private var login: LoginWindowController?
    func applicationDidFinishLaunching(_ notification: Notification) {
        let demo = ProcessInfo.processInfo.arguments.contains("--demo")
        do {
            let storage = try LocalStorage(demo: demo)
            let store = DashboardStore(storage: storage, demo: demo)
            let login = LoginWindowController(store: store)
            let menuBar = MenuBarController(store: store)
            store.openLogin = { [weak login, weak menuBar] in menuBar?.hidePanel(); login?.show() }
            store.closePanel = { [weak menuBar] in menuBar?.hidePanel() }
            login.didClose = { [weak menuBar] in menuBar?.showPanel() }
            self.store = store; self.menuBar = menuBar; self.login = login
            store.start()
            if !demo { store.updater.start() }
        } catch {
            let alert = NSAlert(); alert.messageText = "无法启动 Krill Desktop"
            alert.informativeText = "请检查当前用户的应用支持目录是否可读写。"; alert.runModal(); NSApp.terminate(nil)
        }
    }
    func applicationShouldOpenUntitledFile(_ sender: NSApplication) -> Bool { false }
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool { false }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }
    func applicationWillTerminate(_ notification: Notification) { store?.updater.stop(); store?.stop(); login?.didClose = nil; login?.close() }
}
