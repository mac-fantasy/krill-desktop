import AppKit

@MainActor
enum ApplicationMenu {
    static func make() -> NSMenu {
        let menu = NSMenu()
        let app = NSMenuItem()
        app.submenu = NSMenu(title: "Krill Desktop")
        app.submenu?.addItem(withTitle: "退出 Krill Desktop", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        menu.addItem(app)

        // An AppKit accessory app has no nib-provided Edit menu. Standard key
        // equivalents must route through the responder chain to the field editor.
        let edit = NSMenuItem()
        edit.submenu = NSMenu(title: "编辑")
        let items: [(String, Selector, String)] = [
            ("撤销", Selector(("undo:")), "z"),
            ("重做", Selector(("redo:")), "Z"),
            ("剪切", #selector(NSText.cut(_:)), "x"),
            ("复制", #selector(NSText.copy(_:)), "c"),
            ("粘贴", #selector(NSText.paste(_:)), "v"),
            ("全选", #selector(NSText.selectAll(_:)), "a"),
        ]
        for (title, action, key) in items {
            edit.submenu?.addItem(withTitle: title, action: action, keyEquivalent: key)
        }
        menu.addItem(edit)
        return menu
    }
}
