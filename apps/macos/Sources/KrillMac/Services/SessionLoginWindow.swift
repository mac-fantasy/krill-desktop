import AppKit

@MainActor
final class SessionLoginWindow: NSWindow {
    weak var sessionField: NSSecureTextField?

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        // Accessory apps may have no active Edit-menu target. Dispatch these
        // commands to this window's secure field editor instead of relying on
        // NSApp's key-window lookup (which can still refer to the usage panel).
        let modifiers = event.modifierFlags.intersection([.command, .control, .option, .shift])
        if event.type == .keyDown, modifiers == .command,
           let field = sessionField, !field.isHiddenOrHasHiddenAncestor,
           field.isEnabled, field.isEditable,
           let editor = field.currentEditor() as? NSTextView,
           firstResponder === editor, editor.isEditable {
            switch event.charactersIgnoringModifiers?.lowercased() {
            case "v": editor.paste(nil); return true
            case "a": editor.selectAll(nil); return true
            default: break
            }
        }
        return super.performKeyEquivalent(with: event)
    }
}
