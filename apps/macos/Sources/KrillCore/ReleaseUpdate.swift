import Foundation

/// Release tags deliberately use stable, three-component versions on both platforms.
public struct ReleaseVersion: Comparable, Sendable, CustomStringConvertible {
    public let major: Int
    public let minor: Int
    public let patch: Int
    public init?(_ text: String) {
        let value = text.hasPrefix("v") ? String(text.dropFirst()) : text
        let parts = value.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count == 3 else { return nil }
        let numbers = parts.compactMap { part -> Int? in
            guard !part.isEmpty, part.allSatisfy({ $0.isASCII && $0.isNumber }),
                  part.count == 1 || part.first != "0", let number = Int(part), number <= 65535 else { return nil }
            return number
        }
        guard numbers.count == 3 else { return nil }
        major = numbers[0]; minor = numbers[1]; patch = numbers[2]
    }
    public var description: String { "\(major).\(minor).\(patch)" }
    public static func < (lhs: Self, rhs: Self) -> Bool { (lhs.major, lhs.minor, lhs.patch) < (rhs.major, rhs.minor, rhs.patch) }
}

public struct GitHubRelease: Decodable, Sendable {
    public static let repository = "mac-fantasy/krill-desktop"
    public static let latestURL = URL(string: "https://api.github.com/repos/\(repository)/releases/latest")!
    public let tagName: String
    public let draft: Bool
    public let prerelease: Bool
    public let assets: [Asset]
    enum CodingKeys: String, CodingKey { case tagName = "tag_name", draft, prerelease, assets }

    public struct Asset: Decodable, Sendable {
        public let name: String
        public let browserDownloadURL: URL
        public let digest: String?
        public let size: Int64
        enum CodingKeys: String, CodingKey { case name, browserDownloadURL = "browser_download_url", digest, size }
        public var sha256: String? {
            guard let digest, digest.hasPrefix("sha256:") else { return nil }
            let hex = String(digest.dropFirst(7))
            guard hex.count == 64, hex.allSatisfy({ $0.isASCII && $0.isHexDigit }) else { return nil }
            return hex.lowercased()
        }
    }

    public func update(after current: ReleaseVersion, platform: String) -> Asset? {
        guard !draft, !prerelease, let version = ReleaseVersion(tagName), tagName == "v\(version)", version > current else { return nil }
        let name = "Krill.Desktop-\(version)-\(platform).zip"
        return assets.first { asset in
            asset.name == name && asset.sha256 != nil && asset.size > 0 && asset.size <= 1_073_741_824 &&
            asset.browserDownloadURL.absoluteString == "https://github.com/\(Self.repository)/releases/download/\(tagName)/\(name)"
        }
    }
}
