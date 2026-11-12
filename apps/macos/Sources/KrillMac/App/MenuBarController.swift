import AppKit
import Combine
import KrillCore
import SwiftUI

@MainActor
final class MenuBarController: NSObject, NSPopoverDelegate {
    private let store: DashboardStore
    private let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    private let popover = NSPopover()
    private var subscriptions = Set<AnyCancellable>()

    init(store: DashboardStore) {
        self.store = store
        super.init()
        popover.delegate = self; popover.animates = true
        popover.contentViewController = NSHostingController(rootView: DashboardView(store: store))
        if let button = statusItem.button {
            button.target = self; button.action = #selector(statusClicked(_:))
            button.sendAction(on: [.leftMouseUp, .rightMouseUp]); button.imagePosition = .imageLeading
            button.setAccessibilityLabel("Krill 剩余额度")
        }
        store.objectWillChange.sink { [weak self] _ in
            // ObservableObject sends before mutation; redraw once values have been applied.
            Task { @MainActor [weak self] in self?.updateStatus() }
        }.store(in: &subscriptions)
        updateStatus()
    }
    private func updateStatus() {
        guard let button = statusItem.button else { return }
        button.image = MenuBarIcon.image(percent: store.percent, needsLogin: store.needsLogin, isStale: store.isStale)
        button.title = store.preferences.showPercentage ? " " + store.menuText : ""
        button.toolTip = store.menuTooltip
        button.setAccessibilityValue(store.menuTooltip)
        popover.behavior = store.preferences.panelPinned ? .applicationDefined : .transient
    }
    @objc private func statusClicked(_ sender: Any?) {
        if NSApp.currentEvent?.type == .rightMouseUp { showMenu(); return }
        if popover.isShown { hidePanel() } else { showPanel() }
    }
    func showPanel() {
        guard let button = statusItem.button else { return }
        let available = button.window?.screen?.visibleFrame.height ?? 900
        let size = NSSize(width: 500, height: min(780, max(400, available - 36)))
        popover.contentSize = size
        popover.contentViewController?.preferredContentSize = size
        popover.contentViewController?.view.setFrameSize(size)
        popover.behavior = store.preferences.panelPinned ? .applicationDefined : .transient
        popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
        popover.contentViewController?.view.window?.makeKey()
        Self.clearInitialFocus(in: popover.contentViewController?.view.window)
        NSApp.activate(ignoringOtherApps: true)
        store.isPanelVisible = true; store.synchronizeLaunchStatus(); store.requestRefresh()
    }
    func hidePanel() { popover.performClose(nil); store.isPanelVisible = false }
    func popoverDidClose(_ notification: Notification) { store.isPanelVisible = false }
    func popoverDidShow(_ notification: Notification) {
        Self.clearInitialFocus(in: popover.contentViewController?.view.window)
    }
    static func clearInitialFocus(in window: NSWindow?) {
        // Let Tab choose a control after opening; no button receives focus automatically.
        window?.makeFirstResponder(nil)
    }
    private func showMenu() {
        let menu = NSMenu()
        let entries: [(String, Selector)] = [("打开详情", #selector(openFromMenu)), ("立即刷新", #selector(refreshFromMenu)),
                                             ("检查更新", #selector(updateFromMenu)), ("打开官网", #selector(websiteFromMenu)), ("退出 Krill Desktop", #selector(quitFromMenu))]
        for (title, action) in entries { let item = menu.addItem(withTitle: title, action: action, keyEquivalent: ""); item.target = self }
        statusItem.menu = menu; statusItem.button?.performClick(nil); statusItem.menu = nil
    }
    @objc private func openFromMenu() { showPanel() }
    @objc private func refreshFromMenu() { store.requestRefresh() }
    @objc private func websiteFromMenu() { store.openWebsite() }
    @objc private func updateFromMenu() { store.tab = .settings; showPanel(); store.updater.checkNow() }
    @objc private func quitFromMenu() { NSApp.terminate(nil) }
}
