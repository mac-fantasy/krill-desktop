import AppKit
import Combine
import CryptoKit
import KrillCore

@MainActor
final class UpdateService: ObservableObject {
    @Published private(set) var status = "启动后自动检查更新，每 6 小时检查一次。"
    @Published private(set) var isBusy = false
    @Published private(set) var availableVersion: String?
    let currentVersion = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "开发版"
    private var asset: GitHubRelease.Asset?
    private var task: Task<Void, Never>?
    private var operation: Task<Void, Never>?
    private let session: URLSession = {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForResource = 600
        return URLSession(configuration: configuration)
    }()

    func start() {
        guard task == nil else { return }
        task = Task { [weak self] in
            while !Task.isCancelled {
                await self?.check()
                do { try await Task.sleep(for: .seconds(6 * 60 * 60)) } catch { return }
            }
        }
    }
    func stop() { task?.cancel(); operation?.cancel(); session.invalidateAndCancel() }
    func checkNow() { guard !isBusy else { return }; operation = Task { await check() } }
    private func check() async {
        guard !isBusy else { return }
        guard let current = ReleaseVersion(currentVersion) else { status = "开发模式，请从打包后的应用检查更新。"; return }
        isBusy = true; status = "正在检查 GitHub Release…"
        defer { isBusy = false }
        do {
            var request = URLRequest(url: GitHubRelease.latestURL, timeoutInterval: 30)
            request.setValue("KrillDesktop-Updater/1.0", forHTTPHeaderField: "User-Agent")
            request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
            request.setValue("2022-11-28", forHTTPHeaderField: "X-GitHub-Api-Version")
            let (data, response) = try await session.data(for: request)
            let code = (response as? HTTPURLResponse)?.statusCode
            if code == 404 { status = "暂无可访问的正式版本，请确认发布仓库已公开。"; return }
            guard code == 200 else { throw UpdateFailure("GitHub 暂时不可用或请求受限，请稍后重试。") }
            let release = try JSONDecoder().decode(GitHubRelease.self, from: data)
            asset = release.update(after: current, platform: "macos-universal")
            availableVersion = asset == nil ? nil : ReleaseVersion(release.tagName)?.description
            if let availableVersion { status = "发现新版本 \(availableVersion)，可下载并重启更新。" }
            else if let latest = ReleaseVersion(release.tagName), latest > current, !release.draft, !release.prerelease {
                status = "新版本尚未提供可校验的 macOS 安装包，请稍后重试。"
            } else { status = "当前已是最新版本（\(currentVersion)）。" }
        } catch is CancellationError { }
        catch { status = "检查更新失败，请检查网络后重试。" }
    }
    func install() {
        guard !isBusy, let asset, let version = availableVersion else { return }
        isBusy = true; status = "正在下载 \(version)…"
        operation = Task {
            defer { isBusy = false }
            var archive: URL?
            var staging: URL?
            do {
                let (file, response) = try await session.download(for: URLRequest(url: asset.browserDownloadURL, timeoutInterval: 600))
                archive = file
                guard (response as? HTTPURLResponse)?.statusCode == 200 else { throw UpdateFailure("下载失败，请稍后重试。") }
                status = "正在校验并准备更新…"
                let appURL = Bundle.main.bundleURL
                let result = try await Task.detached {
                    try Self.prepare(archive: file, asset: asset, version: version, appURL: appURL)
                }.value
                staging = result
                try Task.checkCancellation()
                guard let script = Bundle.main.url(forResource: "install-update", withExtension: "sh") else {
                    throw UpdateFailure("当前应用缺少更新组件，请重新下载正式版本。")
                }
                let process = Process(); process.executableURL = URL(fileURLWithPath: "/bin/bash")
                process.arguments = [script.path, String(ProcessInfo.processInfo.processIdentifier), appURL.path, result.path] +
                    Array(ProcessInfo.processInfo.arguments.dropFirst())
                try process.run()
                staging = nil
                NSApp.terminate(nil)
            } catch is CancellationError { }
            catch { status = (error as? UpdateFailure)?.message ?? "更新失败，请确认应用所在目录可写后重试。" }
            if let archive { try? FileManager.default.removeItem(at: archive) }
            if let staging { try? FileManager.default.removeItem(at: staging) }
        }
    }

    nonisolated static func prepare(archive: URL, asset: GitHubRelease.Asset, version: String, appURL: URL) throws -> URL {
        let fm = FileManager.default
        let resolved = appURL.resolvingSymlinksInPath()
        guard resolved.pathExtension == "app", resolved == appURL.standardizedFileURL,
              !resolved.path.contains("/AppTranslocation/"),
              fm.isWritableFile(atPath: resolved.deletingLastPathComponent().path) else {
            throw UpdateFailure("请先将应用移到可写的“应用程序”目录，再执行更新。")
        }
        let size = try fm.attributesOfItem(atPath: archive.path)[.size] as? NSNumber
        guard size?.int64Value == asset.size else { throw UpdateFailure("安装包大小校验失败，请重新下载。") }
        let handle = try FileHandle(forReadingFrom: archive); defer { try? handle.close() }
        var hash = SHA256()
        while let chunk = try handle.read(upToCount: 1_048_576), !chunk.isEmpty { hash.update(data: chunk) }
        let digest = hash.finalize().map { String(format: "%02x", $0) }.joined()
        guard digest == asset.sha256 else { throw UpdateFailure("安装包 SHA-256 校验失败，请重新下载。") }
        let staging = resolved.deletingLastPathComponent().appendingPathComponent(".krill-update-\(UUID().uuidString)", isDirectory: true)
        try fm.createDirectory(at: staging, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
        do {
            try run("/usr/bin/ditto", ["-x", "-k", archive.path, staging.path])
            let app = staging.appendingPathComponent("Krill Desktop.app")
            guard let bundle = Bundle(url: app), bundle.bundleIdentifier == "com.krill.desktop.macos",
                  bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String == version,
                  fm.isExecutableFile(atPath: app.appendingPathComponent("Contents/MacOS/KrillMac").path) else {
                throw UpdateFailure("安装包中的应用或版本不匹配。")
            }
            try run("/usr/bin/codesign", ["--verify", "--strict", app.path])
            return staging
        } catch { try? fm.removeItem(at: staging); throw error }
    }
    nonisolated private static func run(_ executable: String, _ arguments: [String]) throws {
        let process = Process(); process.executableURL = URL(fileURLWithPath: executable); process.arguments = arguments
        process.standardOutput = FileHandle.nullDevice; process.standardError = FileHandle.nullDevice
        try process.run(); process.waitUntilExit()
        guard process.terminationStatus == 0 else { throw UpdateFailure("无法解包或验证安装包，请重新下载。") }
    }
}

private struct UpdateFailure: Error { let message: String; init(_ message: String) { self.message = message } }
