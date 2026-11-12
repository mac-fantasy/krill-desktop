import Foundation
import Security

struct AppPreferences: Codable {
    var selectedSubscriptionID: Int64?
    var lastReadAnnouncementID: Int64 = 0
    var refreshSeconds = 60
    var panelPinned = false
    var showPercentage = false
    var useLiquidGlass = true
    init() {}
    enum CodingKeys: String, CodingKey {
        case selectedSubscriptionID, lastReadAnnouncementID, refreshSeconds, panelPinned, showPercentage, useLiquidGlass
    }
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        selectedSubscriptionID = try c.decodeIfPresent(Int64.self, forKey: .selectedSubscriptionID)
        lastReadAnnouncementID = try c.decodeIfPresent(Int64.self, forKey: .lastReadAnnouncementID) ?? 0
        refreshSeconds = try c.decodeIfPresent(Int.self, forKey: .refreshSeconds) ?? 60
        panelPinned = try c.decodeIfPresent(Bool.self, forKey: .panelPinned) ?? false
        showPercentage = try c.decodeIfPresent(Bool.self, forKey: .showPercentage) ?? false
        useLiquidGlass = try c.decodeIfPresent(Bool.self, forKey: .useLiquidGlass) ?? true
    }
}

@MainActor
final class LocalStorage {
    private let settingsURL: URL
    private let keychainService: String
    init(demo: Bool, directory: URL? = nil) throws {
        let root = try directory ?? FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
            .appendingPathComponent("KrillDesktop", isDirectory: true).appendingPathComponent(demo ? "Demo" : "Account", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        settingsURL = root.appendingPathComponent("settings.json")
        keychainService = demo ? "com.krill.desktop.macos.demo" : "com.krill.desktop.macos.account"
    }
    func loadPreferences() -> AppPreferences {
        guard let data = try? Data(contentsOf: settingsURL), var value = try? JSONDecoder().decode(AppPreferences.self, from: data) else { return AppPreferences() }
        if ![30, 60, 120, 300].contains(value.refreshSeconds) { value.refreshSeconds = 60 }
        return value
    }
    func savePreferences(_ preferences: AppPreferences) throws {
        try JSONEncoder().encode(preferences).write(to: settingsURL, options: .atomic)
    }
    private var query: [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: keychainService,
         kSecAttrAccount as String: "krill-session"]
    }
    func loadSession() throws -> String? {
        var query = query; query[kSecReturnData as String] = true; query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = result as? Data, let value = String(data: data, encoding: .utf8) else { throw StorageFailure.keychain }
        return value
    }
    func saveSession(_ token: String) throws {
        let attributes: [String: Any] = [kSecValueData as String: Data(token.utf8),
                                       kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly]
        let status = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
        if status == errSecItemNotFound {
            var item = query; attributes.forEach { item[$0.key] = $0.value }
            guard SecItemAdd(item as CFDictionary, nil) == errSecSuccess else { throw StorageFailure.keychain }
        } else if status != errSecSuccess { throw StorageFailure.keychain }
    }
    func deleteSession() throws {
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else { throw StorageFailure.keychain }
    }
}

enum StorageFailure: Error { case keychain }
