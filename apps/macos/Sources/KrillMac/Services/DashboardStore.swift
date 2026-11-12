import AppKit
import Combine
import KrillCore
import ServiceManagement

enum DashboardTab: String, CaseIterable, Identifiable {
    case overview = "概览", plans = "套餐", usage = "用量", announcements = "公告", settings = "设置"
    var id: String { rawValue }
}

@MainActor
final class DashboardStore: ObservableObject {
    let updater = UpdateService()
    @Published var tab: DashboardTab = .overview {
        didSet { if tab == .usage || tab == .announcements { requestRefresh() } }
    }
    @Published private(set) var profile: UserProfile?
    @Published private(set) var subscriptions: SubscriptionResponse?
    @Published private(set) var subscriptionDate = Date()
    @Published private(set) var stats: RequestStats?
    @Published private(set) var resetCards: [ResetCard] = []
    @Published private(set) var resetCardsNote = "连接后加载重置卡。"
    @Published private(set) var resetCardsLoaded = false
    @Published private(set) var resetCardsDate = Date()
    @Published private(set) var announcements: [Announcement] = []
    @Published private(set) var isLoading = false
    @Published private(set) var isDemo = false
    @Published private(set) var needsLogin = true
    @Published private(set) var isStale = false
    @Published private(set) var updatedAt: Date?
    @Published private(set) var statusDetail = "使用浏览器或官网任意登录方式连接账号。"
    @Published private(set) var statsNote = "打开面板后自动加载今日统计。"
    @Published private(set) var announcementNote = "连接后加载官网最新公告。"
    @Published var preferences: AppPreferences { didSet { savePreferences() } }
    @Published private(set) var launchAtLogin = false
    @Published private(set) var launchNeedsApproval = false
    var isPanelVisible = false
    var openLogin: (() -> Void)?
    var closePanel: (() -> Void)?
    private let storage: LocalStorage
    private let api = KrillAPI()
    private var token: String?
    private var generation = 0
    private var failures = 0
    private var lastAnnouncements: Date?
    private var lastDetails: Date?
    private var lastAttempt: Date?
    private var nextSubscriptionTransition: Date?
    private var nextResetCardExpiry: Date?
    private var lastResetCards: Date?
    private var pollingTask: Task<Void, Never>?
    private var refreshTask: Task<Void, Never>?
    private var wakeObserver: NSObjectProtocol?
    private let demoOnly: Bool

    init(storage: LocalStorage, demo: Bool) {
        self.storage = storage; self.demoOnly = demo; preferences = storage.loadPreferences()
        synchronizeLaunchStatus()
        if demo { useDemo() }
        else {
            do {
                let stored = try storage.loadSession()
                token = stored.flatMap { KrillOrigin.isPlausibleSession($0) ? $0 : nil }
            } catch { statusDetail = "暂时无法读取钥匙串，解锁后重新连接账号。" }
        }
    }
    var plans: [KrillCore.Subscription] { SubscriptionSelection.ordered(subscriptions?.subscriptions ?? []) }
    var activePlans: [KrillCore.Subscription] { plans.filter { $0.availability(at: subscriptionDate) == .available } }
    var selectedPlan: KrillCore.Subscription? {
        SubscriptionSelection.select(plans, preferredID: preferences.selectedSubscriptionID, at: subscriptionDate)
    }
    var quota: QuotaProjection? { selectedPlan.map { QuotaProjection($0, requestCountQuota: subscriptions?.requestCountQuota) } }
    var resetCardsSummary: String {
        resetCardsLoaded ? "\(resetCards.filter { $0.canUse(at: resetCardsDate) }.count) 张可用 · \(resetCards.count) 张记录" : "—"
    }
    var percent: Double? { quota?.percent }
    var hasData: Bool { subscriptions != nil }
    var accent: ColorStyle { isStale || needsLogin ? .muted : (percent.map { $0 < 20 } ?? false) ? .amber : .mint }
    var primaryRemaining: String { quota.map { $0.amount($0.remaining) } ?? DisplayFormat.money(subscriptions?.creditBalanceUsd) }
    var statusTitle: String { isDemo ? "演示模式" : needsLogin ? (isStale ? "需要重新登录" : "尚未连接") : isStale ? "暂时无法同步" : "已连接" }
    var menuText: String {
        if needsLogin { return "登录" }
        if let percent { return String(format: "%.0f%%", percent) }
        if selectedPlan != nil { return "—" }
        return DisplayFormat.money(subscriptions?.creditBalanceUsd)
    }
    var menuTooltip: String {
        "Krill Desktop\n\(selectedPlan?.plan.name ?? "充值余额") · \(primaryRemaining)\n\(statusTitle) · \(DisplayFormat.date(updatedAt))"
    }
    var unreadCount: Int { announcements.filter { $0.id > preferences.lastReadAnnouncementID }.count }

    func start() {
        requestRefresh()
        pollingTask = Task { [weak self] in
            // Wake frequently to apply panel visibility and preference changes promptly.
            while !Task.isCancelled {
                do { try await Task.sleep(for: .seconds(1)) } catch { return }
                guard let self else { return }
                self.advanceSubscriptionTime()
                self.advanceResetCardTime()
                let interval = min(300, (self.isPanelVisible ? min(30, self.preferences.refreshSeconds) : self.preferences.refreshSeconds) * (1 << min(self.failures, 3)))
                if self.lastAttempt.map({ Date().timeIntervalSince($0) >= Double(interval) }) ?? true {
                    self.requestRefresh()
                }
            }
        }
        wakeObserver = NSWorkspace.shared.notificationCenter.addObserver(forName: NSWorkspace.didWakeNotification, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor [weak self] in self?.requestRefresh() }
        }
    }
    func stop() {
        pollingTask?.cancel(); refreshTask?.cancel()
        if let wakeObserver { NSWorkspace.shared.notificationCenter.removeObserver(wakeObserver) }
        wakeObserver = nil
    }
    func requestRefresh() {
        guard !isLoading, refreshTask == nil, token != nil, !isDemo else { return }
        let current = generation
        refreshTask = Task { [weak self] in
            guard let self else { return }
            await self.refresh()
            if self.generation == current { self.refreshTask = nil }
        }
    }
    private func refresh() async {
        guard let token, !isDemo, !isLoading else { return }
        let current = generation; isLoading = true; lastAttempt = Date()
        defer { if current == generation { isLoading = false } }
        do {
            let account: UserProfile
            if let profile { account = profile } else { account = try await api.profile(token: token) }
            let response = try await api.subscriptions(token: token)
            try Task.checkCancellation()
            guard current == generation else { return }
            profile = account; applySubscriptions(response); updatedAt = Date()
            needsLogin = false; isStale = false; failures = 0; statusDetail = "账号数据已同步，后台会自动更新。"
            if isPanelVisible && (lastResetCards.map { Date().timeIntervalSince($0) >= 30 } ?? true) {
                do {
                    let result = try await api.resetCards(token: token)
                    try Task.checkCancellation()
                    guard current == generation else { return }
                    applyResetCards(result)
                } catch let error as APIFailure where error == .sessionExpired { throw error }
                catch is CancellationError { throw CancellationError() }
                catch { if current == generation { resetCardsNote = "重置卡暂时无法更新，已保留上次结果。" } }
            }
            if isPanelVisible && (lastDetails.map { Date().timeIntervalSince($0) >= 30 } ?? true) {
                do {
                    let result = try await api.stats(token: token)
                    guard current == generation else { return }
                    stats = result; lastDetails = Date(); statsNote = "今日 · 本机时区 · 更新于 \(DisplayFormat.date(Date()))"
                } catch let error as APIFailure where error == .sessionExpired { throw error }
                catch is CancellationError { throw CancellationError() }
                catch { if current == generation { statsNote = "今日统计暂时无法更新，已保留上次结果。" } }
            }
            if lastAnnouncements.map({ Date().timeIntervalSince($0) >= 600 }) ?? true {
                do {
                    let result = try await api.announcements()
                    guard current == generation else { return }
                    announcements = Array(result.items.sorted { ($0.createdAt ?? .distantPast) > ($1.createdAt ?? .distantPast) }.prefix(50))
                    lastAnnouncements = Date(); announcementNote = "官网最新公告 · 点击查看完整内容"
                } catch is CancellationError { throw CancellationError() }
                catch { if current == generation { announcementNote = "公告暂时无法更新，可稍后刷新或在官网查看。" } }
            }
        } catch is CancellationError { }
        catch {
            guard current == generation else { return }
            let failure = error as? APIFailure ?? .network
            failures += 1; isStale = true; statusDetail = failure.message
            if failure == .sessionExpired {
                self.token = nil; needsLogin = true
                do { try storage.deleteSession() } catch { statusDetail += " 本机旧会话未能清除，请重新连接。" }
            }
        }
    }
    func loginRequested() { openLogin?() }
    func applyResetCards(_ response: ResetCardsResponse, at now: Date = Date()) {
        resetCards = response.items; resetCardsLoaded = true; resetCardsDate = now; lastResetCards = now
        resetCardsNote = "重置卡只恢复适用订阅的本周额度；使用与预览请前往官网。"
        nextResetCardExpiry = resetCards.compactMap { $0.expiresAt }.filter { $0 > now }.min()
    }
    func advanceResetCardTime(at now: Date = Date()) {
        guard let nextResetCardExpiry, now >= nextResetCardExpiry else { return }
        resetCardsDate = now
        self.nextResetCardExpiry = resetCards.compactMap { $0.expiresAt }.filter { $0 > now }.min()
    }
    func applySubscriptions(_ response: SubscriptionResponse, at now: Date = Date()) {
        let wasMonthly = selectedPlan?.isMonthly ?? false
        subscriptions = response
        reconcileSelection(at: now, preferredWasMonthly: wasMonthly)
    }
    func advanceSubscriptionTime(at now: Date = Date()) {
        guard let nextSubscriptionTransition, now >= nextSubscriptionTransition else { return }
        reconcileSelection(at: now, preferredWasMonthly: selectedPlan?.isMonthly ?? false)
    }
    private func reconcileSelection(at now: Date, preferredWasMonthly: Bool) {
        subscriptionDate = now
        let selected = SubscriptionSelection.select(plans, preferredID: preferences.selectedSubscriptionID,
                                                     preferredWasMonthly: preferredWasMonthly, at: now)
        if preferences.selectedSubscriptionID != selected?.id { preferences.selectedSubscriptionID = selected?.id }
        nextSubscriptionTransition = SubscriptionSelection.nextTransition(in: plans, after: now)
    }
    // Browser extraction and explicit user imports both require API verification before persistence.
    func acceptSession(_ candidate: String) async throws {
        guard !demoOnly else { throw APIFailure.forbidden }
        let current = generation
        let account = try await api.profile(token: candidate)
        try Task.checkCancellation()
        guard generation == current else { throw CancellationError() }
        try storage.saveSession(candidate)
        resetSession(); clearView(); token = candidate; profile = account; needsLogin = false
        requestRefresh()
    }
    func signOut() {
        resetSession(); token = nil; clearView()
        do { try storage.deleteSession() } catch { statusDetail = "本机钥匙串会话未能清除，请检查钥匙串访问权限。"; isStale = true }
    }
    func useDemo() {
        resetSession(); token = nil; isDemo = true; needsLogin = false; isStale = false
        profile = DemoData.profile; applySubscriptions(DemoData.subscriptions()); stats = DemoData.stats()
        applyResetCards(DemoData.resetCards())
        announcements = DemoData.announcements(); updatedAt = Date(); statusDetail = "虚构样例，不代表真实账号。"
        statsNote = "今日 · 演示数据"; announcementNote = "虚构演示公告"
    }
    private func resetSession() {
        generation += 1; refreshTask?.cancel(); refreshTask = nil; isLoading = false; failures = 0; lastAnnouncements = nil; lastDetails = nil; lastResetCards = nil
    }
    private func clearView() {
        profile = nil; subscriptions = nil; nextSubscriptionTransition = nil; stats = nil; announcements = []; updatedAt = nil
        resetCards = []; resetCardsLoaded = false; nextResetCardExpiry = nil; resetCardsNote = "连接后加载重置卡。"
        isDemo = false; needsLogin = true; isStale = false; tab = .overview
        statusDetail = "使用浏览器或官网任意登录方式连接账号。"; statsNote = "连接账号后加载今日统计。"
    }
    private func savePreferences() {
        do { try storage.savePreferences(preferences) } catch { statusDetail = "本次设置暂时无法保存，请检查应用数据目录。" }
    }
    func openWebsite(_ path: WebsitePath = .account) {
        NSWorkspace.shared.open(KrillOrigin.url.appendingPathComponent(String(path.rawValue.dropFirst())))
    }
    func openAnnouncement(_ item: Announcement) {
        preferences.lastReadAnnouncementID = max(preferences.lastReadAnnouncementID, item.id)
        openWebsite(.announcements)
    }
    func synchronizeLaunchStatus() {
        launchAtLogin = SMAppService.mainApp.status == .enabled
        launchNeedsApproval = SMAppService.mainApp.status == .requiresApproval
    }
    func setLaunchAtLogin(_ enabled: Bool) {
        do {
            if enabled { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
            synchronizeLaunchStatus()
            if launchNeedsApproval { statusDetail = "请在系统设置的“登录项”中允许 Krill Desktop。" }
        } catch { synchronizeLaunchStatus(); statusDetail = "无法修改登录项。请从打包后的 .app 运行，再检查系统设置。" }
    }
    var isDemoOnly: Bool { demoOnly }
}

enum WebsitePath: String { case account = "/app", plans = "/app/shop", logs = "/app/logs", announcements = "/app/announcements" }
enum ColorStyle { case mint, amber, muted }
