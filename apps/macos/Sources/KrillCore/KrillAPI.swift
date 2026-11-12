import Foundation

public enum APIFailure: Error, Sendable, Equatable {
    case sessionExpired, forbidden, rateLimited, network, service, invalidResponse
    public var message: String {
        switch self {
        case .sessionExpired: "登录已失效，请重新连接账号。"
        case .forbidden: "当前账号暂时无权读取此项数据。"
        case .rateLimited: "请求较频繁，稍后会自动重试。"
        case .network: "暂时无法连接，已保留上次成功读取的数据。"
        case .service: "网站暂时不可用，稍后会自动重试。"
        case .invalidResponse: "网站返回的数据格式有变化，请稍后重试。"
        }
    }
}

public enum KrillEndpoint: String, Sendable {
    case profile = "/api/auth/me", subscriptions = "/api/subscription"
    case stats = "/api/request-logs/stats", announcements = "/api/public/announcements"
    case resetCards = "/api/reset-cards"
}

public enum KrillOrigin {
    public static let url = URL(string: "https://www.krill-code.com")!
    public static func isTrusted(_ url: URL?) -> Bool {
        guard let url else { return false }
        return url.scheme?.lowercased() == "https" && url.host?.lowercased() == "www.krill-code.com"
            && (url.port == nil || url.port == 443) && url.user == nil && url.password == nil
    }
    public static func allowsLoginNavigation(_ url: URL?) -> Bool {
        guard let url else { return false }
        // Identity providers can delegate to another provider (for example GitHub to Apple).
        // Navigation is broader than the exact origin allowed to supply a Krill credential.
        return url.scheme?.lowercased() == "https" && url.host?.isEmpty == false
            && (url.port == nil || url.port == 443) && url.user == nil && url.password == nil
    }
    public static func isPlausibleSession(_ token: String) -> Bool {
        (16...16384).contains(token.utf8.count) && token.range(of: #"\A[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\z"#, options: .regularExpression) != nil
    }
}

private final class NoRedirectDelegate: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest, completionHandler: @escaping @Sendable (URLRequest?) -> Void) {
        completionHandler(nil)
    }
}

private struct ResponseStatus: Decodable { let code: Int? }

public actor KrillAPI {
    private let session: URLSession
    public init() {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.httpShouldSetCookies = false; configuration.httpCookieStorage = nil
        configuration.urlCache = nil; configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        configuration.timeoutIntervalForRequest = 20; configuration.timeoutIntervalForResource = 30
        session = URLSession(configuration: configuration, delegate: NoRedirectDelegate(), delegateQueue: nil)
    }
    public static func request(_ endpoint: KrillEndpoint, token: String? = nil, body: Data? = nil) throws -> URLRequest {
        var request = URLRequest(url: KrillOrigin.url.appendingPathComponent(String(endpoint.rawValue.dropFirst())))
        request.httpMethod = endpoint == .stats ? "POST" : "GET"
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("zh-CN", forHTTPHeaderField: "X-Language")
        request.setValue("zh-CN", forHTTPHeaderField: "Accept-Language")
        request.setValue("KrillDesktop-macOS", forHTTPHeaderField: "User-Agent")
        if let token {
            guard KrillOrigin.isPlausibleSession(token) else { throw APIFailure.sessionExpired }
            request.setValue("Bearer " + token, forHTTPHeaderField: "Authorization")
            request.setValue("krill_jwt=" + token, forHTTPHeaderField: "Cookie")
        }
        if let body { request.httpBody = body; request.setValue("application/json", forHTTPHeaderField: "Content-Type") }
        return request
    }
    private func send<T: Decodable & Sendable>(_ type: T.Type, endpoint: KrillEndpoint, token: String? = nil, body: Data? = nil) async throws -> T {
        do {
            let (bytes, response) = try await session.bytes(for: Self.request(endpoint, token: token, body: body))
            guard let response = response as? HTTPURLResponse else { throw APIFailure.invalidResponse }
            switch response.statusCode {
            case 401: throw APIFailure.sessionExpired
            case 403: throw APIFailure.forbidden
            case 429: throw APIFailure.rateLimited
            case 200...299: break
            default: throw APIFailure.service
            }
            var data = Data()
            for try await byte in bytes {
                try Task.checkCancellation()
                guard data.count < 8 * 1024 * 1024 else { throw APIFailure.invalidResponse }
                data.append(byte)
            }
            // Inspect business code before decoding Data: a 401 response may carry an object with a different shape.
            if try KrillJSON.decoder().decode(ResponseStatus.self, from: data).code == 401 { throw APIFailure.sessionExpired }
            let envelope = try KrillJSON.decoder().decode(APIEnvelope<T>.self, from: data)
            guard envelope.success, let value = envelope.data else { throw APIFailure.invalidResponse }
            return value
        } catch is CancellationError { throw CancellationError() }
        catch let error as APIFailure { throw error }
        catch let error as URLError {
            if error.code == .cancelled { throw CancellationError() }
            throw APIFailure.network
        } catch is DecodingError { throw APIFailure.invalidResponse }
        catch { throw APIFailure.network }
    }
    public func profile(token: String) async throws -> UserProfile { try await send(UserProfile.self, endpoint: .profile, token: token) }
    public func subscriptions(token: String) async throws -> SubscriptionResponse { try await send(SubscriptionResponse.self, endpoint: .subscriptions, token: token) }
    public func resetCards(token: String) async throws -> ResetCardsResponse { try await send(ResetCardsResponse.self, endpoint: .resetCards, token: token) }
    public func stats(token: String, now: Date = Date()) async throws -> RequestStats {
        let range = DisplayFormat.dayRange(now: now)
        let f = ISO8601DateFormatter(); f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let body = try JSONSerialization.data(withJSONObject: ["start_time": f.string(from: range.from), "end_time": f.string(from: range.to)])
        return try await send(RequestStats.self, endpoint: .stats, token: token, body: body)
    }
    public func announcements() async throws -> AnnouncementsResponse { try await send(AnnouncementsResponse.self, endpoint: .announcements) }
}
