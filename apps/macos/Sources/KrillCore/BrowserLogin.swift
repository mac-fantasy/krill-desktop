import Foundation

public enum BrowserLogin {
    public static let loginURL = KrillOrigin.url.appendingPathComponent("login")
    // A prompt works in Safari as well as Chromium and Firefox, without a browser-specific copy() API.
    public static let exportScript = """
    (() => {
      if (location.origin !== 'https://www.krill-code.com') { alert('请在 Krill 官网运行'); return; }
      const token = localStorage.getItem('krill_jwt');
      if (!token) { alert('请先完成 Krill 登录'); return; }
      prompt('复制 Krill 会话并粘贴到 Krill Desktop（请勿分享给他人）', token);
    })()
    """

    public static func session(from text: String) -> String? {
        guard text.utf8.count <= 16400 else { return nil }
        var value = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if value.hasPrefix("Bearer ") { value = String(value.dropFirst(7)) }
        return KrillOrigin.isPlausibleSession(value) ? value : nil
    }
}
