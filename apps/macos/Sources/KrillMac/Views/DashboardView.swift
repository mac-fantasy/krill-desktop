import AppKit
import KrillCore
import ServiceManagement
import SwiftUI

struct DashboardView: View {
    @ObservedObject var store: DashboardStore
    @State private var confirmSignOut = false

    var body: some View {
        VStack(spacing: 0) {
            header
            navigation.padding(.horizontal, 24)
            UpdateNotice(updater: store.updater)
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    if store.isDemo { notice("演示数据", "虚构样例，不代表真实账号。", color: KrillTheme.amber) }
                    if store.isStale {
                        notice(store.statusTitle, store.statusDetail, color: KrillTheme.amber)
                        if store.needsLogin { connectButton }
                    }
                    if !store.hasData && store.tab != .settings { welcome }
                    else {
                        switch store.tab {
                        case .overview: overview
                        case .plans: plans
                        case .usage: usage
                        case .announcements: announcements
                        case .settings: settings
                        }
                    }
                }.padding(.horizontal, 24).padding(.top, 18).padding(.bottom, 24)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }.scrollIndicators(.automatic)
            footer
        }.frame(minWidth: 460, idealWidth: 500, maxWidth: .infinity, maxHeight: .infinity)
            .background(KrillTheme.background).foregroundStyle(.white)
            .preferredColorScheme(.dark).tint(KrillTheme.mint)
            .font(.system(size: 13)).buttonStyle(.plain)
            .onExitCommand { store.closePanel?() }
            .confirmationDialog("退出当前账号？", isPresented: $confirmSignOut) {
                Button("退出账号", role: .destructive) { store.signOut() }
                Button("取消", role: .cancel) { }
            } message: { Text("将清除本机钥匙串中的 Krill 会话。官网中的登录状态独立保留。") }
    }
    private var header: some View {
        HStack(spacing: 11) {
            Text("k").font(.system(size: 29, weight: .bold, design: .rounded))
                .foregroundStyle(Color(red: 0.07, green: 0.23, blue: 0.20))
                .frame(width: 36, height: 36).background(KrillTheme.mint, in: RoundedRectangle(cornerRadius: 11))
            VStack(alignment: .leading, spacing: 0) {
                Text("krill").font(.system(size: 24, weight: .semibold))
                Text("D E S K T O P").font(.system(size: 8)).foregroundStyle(KrillTheme.muted)
            }
            Spacer()
            HStack(spacing: 3) {
                iconButton("arrow.clockwise", title: "立即刷新", disabled: store.isLoading || store.isDemo || store.needsLogin) { store.requestRefresh() }
                iconButton(store.preferences.panelPinned ? "pin.fill" : "pin", title: "固定或取消固定面板") { store.preferences.panelPinned.toggle() }
                iconButton("xmark", title: "收起面板") { store.closePanel?() }
            }.padding(3).modifier(GlassSurface(enabled: store.preferences.useLiquidGlass))
        }.padding(.horizontal, 24).frame(height: 76)
            .overlay(alignment: .bottom) { if store.isLoading { ProgressView().progressViewStyle(.linear).frame(height: 2).padding(.horizontal, 24) } }
    }
    private var navigation: some View {
        HStack(spacing: 2) {
            ForEach(DashboardTab.allCases) { tab in
                Button { store.tab = tab } label: {
                    Text(tab.rawValue).font(.system(size: 12, weight: store.tab == tab ? .semibold : .regular))
                        .frame(maxWidth: .infinity, minHeight: 40)
                        .foregroundStyle(store.tab == tab ? KrillTheme.mint : KrillTheme.muted)
                        .background(store.tab == tab ? KrillTheme.mint.opacity(0.12) : .clear, in: RoundedRectangle(cornerRadius: 9))
                        .contentShape(Rectangle())
                }.accessibilityAddTraits(store.tab == tab ? .isSelected : [])
            }
        }.padding(4).modifier(GlassSurface(enabled: store.preferences.useLiquidGlass))
    }
    private var welcome: some View {
        VStack(spacing: 20) {
            Image(systemName: "circle.dotted.circle.fill").font(.system(size: 54)).foregroundStyle(KrillTheme.mint).padding(.top, 35)
            Text("你的桌面用量助手").font(.system(size: 24, weight: .semibold))
            Text("连接 Krill 账号，在菜单栏随时掌握套餐、余额与今日用量。")
                .multilineTextAlignment(.center).foregroundStyle(KrillTheme.muted).lineSpacing(5)
            if store.isDemoOnly { Button("查看演示") { store.useDemo() }.buttonStyle(.borderedProminent) }
            else { connectButton; Button("先看看演示") { store.useDemo() }.foregroundStyle(KrillTheme.muted) }
            Text("登录在官网原有页面完成，会话保存在 macOS 钥匙串。")
                .font(.system(size: 11)).foregroundStyle(KrillTheme.muted).multilineTextAlignment(.center)
        }.frame(maxWidth: .infinity).padding(.vertical, 30)
    }
    private var connectButton: some View {
        Button("连接 Krill 账号") { store.loginRequested() }
            .buttonStyle(.borderedProminent).controlSize(.large).disabled(store.isDemoOnly)
    }
    private var overview: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(spacing: 12) {
                Text(store.profile.map { String($0.friendlyName.prefix(1)) } ?? "K").font(.system(size: 18, weight: .semibold))
                    .frame(width: 42, height: 42).background(KrillTheme.mint.opacity(0.12), in: RoundedRectangle(cornerRadius: 13))
                VStack(alignment: .leading, spacing: 5) {
                    Text(store.profile?.friendlyName ?? "Krill 用户").font(.system(size: 17, weight: .semibold))
                    Text(store.profile?.email ?? "已连接 Krill 账号").font(.system(size: 11)).foregroundStyle(KrillTheme.muted)
                }
                Spacer()
            }.padding(.bottom, 4)
            Card {
                HStack(alignment: .center, spacing: 22) {
                    VStack(alignment: .leading, spacing: 9) {
                        Text(store.selectedPlan?.plan.name ?? "按余额使用").font(.system(size: 14, weight: .semibold))
                        Text(store.quota?.cycle ?? "充值余额").font(.system(size: 11)).foregroundStyle(KrillTheme.muted)
                        Text(store.primaryRemaining).font(.system(size: 34, weight: .semibold, design: .rounded)).monospacedDigit()
                            .foregroundStyle(store.accent.color).lineLimit(1).minimumScaleFactor(0.6)
                        Text(store.quota.map { "已用 \($0.amount($0.used))  /  上限 \($0.amount($0.limit))" } ?? "余额按实际使用扣除")
                            .font(.system(size: 11)).foregroundStyle(KrillTheme.muted)
                    }.frame(maxWidth: .infinity, alignment: .leading)
                    QuotaRing(percent: store.percent, color: store.accent.color)
                }
                Divider().overlay(KrillTheme.border)
                HStack {
                    Text(store.quota?.resetAt.map { "\(DisplayFormat.date($0)) 重置" } ?? "周期时间以官网为准")
                    Spacer()
                    if let forwarded = store.quota?.forwarded, forwarded > 0 { Text("另有结转 \(DisplayFormat.money(forwarded))").foregroundStyle(KrillTheme.mint) }
                }.font(.system(size: 10)).foregroundStyle(KrillTheme.muted)
            }
            HStack(spacing: 12) {
                Metric(title: "充值余额 / USD", value: DisplayFormat.money(store.subscriptions?.creditBalanceUsd))
                Metric(title: "福利余额 / USD", value: DisplayFormat.money(store.subscriptions?.welfareBalanceUsd))
            }
            sectionHeader("今日用量", action: "查看详情") { store.tab = .usage }
            HStack(spacing: 10) {
                Metric(title: "请求数", value: store.stats.map { DisplayFormat.compact($0.totalRequests) } ?? "—")
                Metric(title: "Tokens", value: store.stats.map { DisplayFormat.compact($0.totalTokens) } ?? "—")
                Metric(title: "费用 / USD", value: DisplayFormat.cost(store.stats?.totalCostUsd), accent: true)
            }
            HStack(spacing: 10) {
                Metric(title: "缓存率", value: DisplayFormat.percentage(store.stats?.cachePercent))
                Metric(title: "可用重置卡", value: store.resetCardsLoaded ? String(store.resetCards.filter { $0.canUse(at: store.resetCardsDate) }.count) + " 张" : "—")
            }
            sectionHeader("最新公告", action: "全部公告") { store.tab = .announcements }
            if store.announcements.isEmpty { Text(store.announcementNote).foregroundStyle(KrillTheme.muted).font(.system(size: 12)) }
            ForEach(Array(store.announcements.prefix(2))) { announcementCard($0) }
        }
    }
    private var plans: some View {
        VStack(alignment: .leading, spacing: 16) {
            sectionHeader("我的套餐", action: "管理套餐 ↗") { store.openWebsite(.plans) }
            Text("\(store.activePlans.count) 个有效套餐").foregroundStyle(KrillTheme.muted)
            Text("菜单栏显示的套餐").font(.system(size: 11)).foregroundStyle(KrillTheme.muted)
            Picker("菜单栏显示的套餐", selection: Binding<Int64?>(get: { store.selectedPlan?.id }, set: { store.preferences.selectedSubscriptionID = $0 })) {
                if store.activePlans.isEmpty { Text("按余额使用").tag(Optional<Int64>.none) }
                ForEach(store.activePlans) { plan in Text(plan.plan.name).tag(Optional(plan.id)) }
            }.labelsHidden().pickerStyle(.menu).frame(maxWidth: .infinity, alignment: .leading)
            ForEach(store.plans) { planCard($0) }
            resetCardsSection
            Text("金额单位为 USD。结转额度单独显示；套餐变更、续费与支付请前往官网。")
                .font(.system(size: 11)).foregroundStyle(KrillTheme.muted).lineSpacing(5)
        }
    }
    private func planCard(_ plan: Subscription) -> some View {
        let quota = QuotaProjection(plan, requestCountQuota: store.subscriptions?.requestCountQuota)
        let state = plan.availability(at: store.subscriptionDate)
        let available = state == .available
        let accent = available ? KrillTheme.mint : KrillTheme.muted
        return Card {
            HStack(alignment: .top) {
                Text(plan.plan.name).fontWeight(.semibold)
                Spacer()
                Text(state.label)
                    .font(.system(size: 10)).foregroundStyle(accent).padding(.horizontal, 8).padding(.vertical, 5)
                    .background(accent.opacity(0.10), in: RoundedRectangle(cornerRadius: 6))
            }
            Text(quota.cycle).font(.system(size: 11)).foregroundStyle(KrillTheme.muted)
            HStack {
                Text(quota.amount(quota.remaining)).font(.system(size: 28, weight: .semibold, design: .rounded)).foregroundStyle(accent).monospacedDigit()
                Spacer(); Text("上限 \(quota.amount(quota.limit))").font(.system(size: 11)).foregroundStyle(KrillTheme.muted)
            }
            ProgressView(value: quota.percent ?? 0, total: 100).tint(accent)
                .accessibilityLabel("剩余额度").accessibilityValue(quota.percent.map { String(format: "%.0f%%", $0) } ?? "未知")
            Text("已用 \(quota.amount(quota.used))").font(.system(size: 11)).foregroundStyle(KrillTheme.muted)
            if plan.plan.billingType == "request_count" {
                Text("狂蹬卡 / 按次额度 · 账号共享限额").font(.system(size: 11)).foregroundStyle(KrillTheme.muted)
                ForEach(requestWindows(for: plan)) { window in
                    detailRow(window.label, value: window.detail)
                    ProgressView(value: window.percent ?? 0, total: 100).tint(accent)
                        .accessibilityLabel(window.label + "剩余请求额度").accessibilityValue(window.percent.map { String(format: "%.1f%%", $0) } ?? "未知")
                }
                Text(store.subscriptions?.requestCountQuota?.cycle ?? "周期时间未提供").font(.system(size: 10)).foregroundStyle(KrillTheme.muted)
            }
            if let forwarded = quota.forwarded, forwarded > 0 { Text("另有结转 \(DisplayFormat.money(forwarded))").font(.system(size: 11)).foregroundStyle(accent) }
            Divider()
            HStack {
                Text(quota.resetAt.map { "\(DisplayFormat.date($0)) 重置" } ?? "周期时间以官网为准")
                Spacer(); Text("\(DisplayFormat.date(plan.subscriptionEndAt)) 到期")
            }.font(.system(size: 10)).foregroundStyle(KrillTheme.muted)
        }.saturation(available ? 1 : 0).opacity(available ? 1 : 0.55)
    }
    private func requestWindows(for plan: Subscription) -> [RequestQuotaWindow] {
        store.subscriptions?.requestCountQuota?.windows ?? [
            RequestQuotaWindow(label: "5 小时", used: nil, limit: plan.plan.rateLimit5h),
            RequestQuotaWindow(label: "本周", used: nil, limit: plan.plan.rateLimitWeekly),
            RequestQuotaWindow(label: "本月", used: nil, limit: plan.plan.rateLimitMonthly)
        ]
    }
    private var resetCardsSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            sectionHeader("重置卡", action: "官网管理 ↗") { store.openWebsite() }
            Text(store.resetCardsSummary).foregroundStyle(KrillTheme.mint)
            Text(store.resetCardsNote).font(.system(size: 11)).foregroundStyle(KrillTheme.muted)
            if store.resetCardsLoaded && store.resetCards.isEmpty { Text("暂无重置卡").foregroundStyle(KrillTheme.muted) }
            ForEach(store.resetCards) { card in
                Card {
                    HStack {
                        Text(card.planName ?? "重置卡").fontWeight(.semibold)
                        Spacer(); Text("#\(card.id)").foregroundStyle(KrillTheme.muted)
                    }
                    Text(card.availabilityText(at: store.resetCardsDate)).foregroundStyle(card.canUse(at: store.resetCardsDate) ? KrillTheme.mint : KrillTheme.muted)
                    detailRow("单张恢复上限", value: DisplayFormat.money(card.capUsd))
                    detailRow("有效期", value: card.expiresAt.map { DisplayFormat.date($0) + " 前有效" } ?? "长期有效")
                    if card.state(at: store.resetCardsDate) == .used {
                        detailRow("实际恢复", value: DisplayFormat.money(card.grantedUsd))
                        detailRow("使用时间", value: DisplayFormat.date(card.usedAt))
                    }
                    if let note = card.note, !note.isEmpty { Text(DisplayFormat.plainText(note)).font(.system(size: 11)).foregroundStyle(KrillTheme.muted) }
                }.opacity(card.state(at: store.resetCardsDate) == .unused ? 1 : 0.55)
            }
        }
    }
    private var usage: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("今日用量").font(.system(size: 23, weight: .semibold))
            Text(store.statsNote).foregroundStyle(KrillTheme.muted).font(.system(size: 12))
            HStack(spacing: 10) {
                Metric(title: "请求数", value: store.stats.map { DisplayFormat.compact($0.totalRequests) } ?? "—")
                Metric(title: "Tokens", value: store.stats.map { DisplayFormat.compact($0.totalTokens) } ?? "—")
                Metric(title: "费用 / USD", value: DisplayFormat.cost(store.stats?.totalCostUsd), accent: true)
            }
            Card {
                Text("时段费用").fontWeight(.semibold)
                UsageTrendChart(buckets: store.stats?.trend ?? [])
            }
            Card {
                detailRow("请求成功率", value: store.stats.flatMap { $0.totalRequests > 0 ? String(format: "%.1f%%", Double($0.successRequests) / Double($0.totalRequests) * 100) : nil } ?? "—")
                detailRow("成功请求", value: store.stats.map { DisplayFormat.optionalCount($0.successRequests) } ?? "—")
                detailRow("失败请求", value: DisplayFormat.optionalCount(store.stats?.failedRequests))
                detailRow("每分钟请求 / RPM", value: DisplayFormat.rate(store.stats?.rpm))
                detailRow("每分钟 Tokens / TPM", value: DisplayFormat.rate(store.stats?.tpm))
                detailRow("输入 Tokens", value: DisplayFormat.optionalCount(store.stats?.inputTokens))
                detailRow("输出 Tokens", value: store.stats.map { DisplayFormat.compact($0.outputTokens) } ?? "—")
                detailRow("思考 Tokens", value: DisplayFormat.optionalCount(store.stats?.reasoningTokens))
            }
            Card {
                Text("缓存用量").fontWeight(.semibold)
                detailRow("缓存率", value: DisplayFormat.percentage(store.stats?.cachePercent))
                detailRow("缓存读取 Tokens", value: DisplayFormat.optionalCount(store.stats?.cacheReadInputTokens))
                detailRow("缓存写入 Tokens", value: DisplayFormat.optionalCount(store.stats?.cacheCreationInputTokens))
                Text("优先使用官网缓存统计口径；缺失时按缓存读取 /（输入 + 缓存写入 + 缓存读取）计算。").font(.system(size: 10)).foregroundStyle(KrillTheme.muted)
                ForEach(Array((store.stats?.channelCacheRates ?? []).enumerated()), id: \.offset) { _, channel in
                    detailRow(DisplayFormat.plainText(channel.channelName, limit: 60), value: DisplayFormat.percentage(channel.cacheRate.map { $0 * 100 }))
                }
            }
            Button("在官网查看完整用量记录 ↗") { store.openWebsite(.logs) }.foregroundStyle(KrillTheme.mint)
        }
    }
    private var announcements: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack { Text("公告与动态").font(.system(size: 23, weight: .semibold)); Spacer(); Text(store.unreadCount > 0 ? "\(store.unreadCount) 条未读" : "全部已读").font(.system(size: 11)).foregroundStyle(KrillTheme.mint) }
            Text(store.announcementNote).foregroundStyle(KrillTheme.muted).font(.system(size: 12))
            ForEach(store.announcements) { announcementCard($0) }
        }
    }
    private func announcementCard(_ item: Announcement) -> some View {
        Button { store.openAnnouncement(item) } label: {
            Card {
                HStack {
                    Text(DisplayFormat.date(item.createdAt)).foregroundStyle(KrillTheme.muted)
                    if item.id > store.preferences.lastReadAnnouncementID { Text("• 未读").foregroundStyle(KrillTheme.mint) }
                    Spacer(); Image(systemName: "arrow.up.right").foregroundStyle(KrillTheme.muted)
                }.font(.system(size: 10))
                Text(DisplayFormat.plainText(item.title, limit: 80)).fontWeight(.semibold).multilineTextAlignment(.leading)
                Text(DisplayFormat.plainText(item.summary ?? item.content)).font(.system(size: 12)).foregroundStyle(KrillTheme.muted).lineSpacing(5).multilineTextAlignment(.leading)
            }
        }
    }
    private var settings: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("按你的习惯来").font(.system(size: 23, weight: .semibold))
            Text("设置自动保存，仅作用于当前设备。").font(.system(size: 12)).foregroundStyle(KrillTheme.muted)
            Card {
                Text("菜单栏与面板").font(.system(size: 11)).foregroundStyle(KrillTheme.muted)
                Toggle("在菜单栏图标旁显示百分比或余额", isOn: $store.preferences.showPercentage)
                Toggle("固定面板，点击其他窗口时不收起", isOn: $store.preferences.panelPinned)
                Toggle("使用 Liquid Glass（macOS 26 及以上）", isOn: $store.preferences.useLiquidGlass)
                Toggle("登录 macOS 后自动启动", isOn: Binding(get: { store.launchAtLogin }, set: { store.setLaunchAtLogin($0) }))
                if store.launchNeedsApproval {
                    Button("在系统设置中允许登录项") { SMSettings.openLoginItems() }.foregroundStyle(KrillTheme.mint)
                }
            }.toggleStyle(.switch).controlSize(.small)
            Card {
                Picker("后台刷新间隔", selection: $store.preferences.refreshSeconds) {
                    ForEach([30, 60, 120, 300], id: \.self) { Text("\($0) 秒").tag($0) }
                }.pickerStyle(.menu)
                Text("展开面板时最多每 30 秒同步核心数据；公告约每 10 分钟检查一次。")
                    .font(.system(size: 11)).foregroundStyle(KrillTheme.muted)
            }
            Card {
                Text("账号与连接").fontWeight(.semibold)
                Text("会话保存在 macOS 钥匙串。退出账号会清除本机会话，官网中的登录状态独立保留。")
                    .font(.system(size: 12)).foregroundStyle(KrillTheme.muted).lineSpacing(4)
                HStack(spacing: 16) {
                    Button("连接账号") { store.loginRequested() }.buttonStyle(.borderedProminent).disabled(store.isDemoOnly)
                    if !store.needsLogin { Button("退出账号", role: .destructive) { confirmSignOut = true } }
                }
            }
            UpdateSettings(updater: store.updater)
            HStack {
                Text("KRILL DESKTOP / macOS · \(store.updater.currentVersion)").font(.system(size: 10)).foregroundStyle(KrillTheme.muted)
                Spacer(); Button("退出程序") { NSApp.terminate(nil) }.foregroundStyle(Color(red: 0.86, green: 0.67, blue: 0.65))
            }
        }
    }
    private var footer: some View {
        HStack {
            VStack(alignment: .leading, spacing: 5) {
                HStack(spacing: 6) { Circle().fill(store.accent.color).frame(width: 5, height: 5); Text(store.statusTitle).font(.system(size: 11)) }
                Text(store.updatedAt.map { "更新于 \(DisplayFormat.date($0))" } ?? "等待首次同步").font(.system(size: 10)).foregroundStyle(KrillTheme.muted)
            }.help(store.statusDetail)
            Spacer()
            Button("打开官网  ↗") { store.openWebsite() }.font(.system(size: 12)).padding(.horizontal, 15).padding(.vertical, 9)
                .foregroundStyle(KrillTheme.mint).modifier(GlassSurface(enabled: store.preferences.useLiquidGlass))
        }.padding(.vertical, 14).padding(.horizontal, 24)
            .overlay(alignment: .top) { Rectangle().fill(KrillTheme.border).frame(height: 1).padding(.horizontal, 24) }
    }
    private func iconButton(_ symbol: String, title: String, disabled: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) { Image(systemName: symbol).frame(width: 29, height: 29).contentShape(Rectangle()) }
            .help(title).accessibilityLabel(title).disabled(disabled).opacity(disabled ? 0.4 : 1)
    }
    private func sectionHeader(_ title: String, action: String, perform: @escaping () -> Void) -> some View {
        HStack { Text(title).font(.system(size: 16, weight: .semibold)); Spacer(); Button(action, action: perform).font(.system(size: 11)).foregroundStyle(KrillTheme.mint) }
    }
    private func detailRow(_ title: String, value: String) -> some View {
        HStack { Text(title).foregroundStyle(KrillTheme.muted); Spacer(); Text(value).monospacedDigit() }.font(.system(size: 12))
    }
    private func notice(_ title: String, _ text: String, color: Color) -> some View {
        VStack(alignment: .leading, spacing: 6) { Text(title).fontWeight(.semibold); Text(text).font(.system(size: 12)).lineSpacing(4) }
            .foregroundStyle(color).frame(maxWidth: .infinity, alignment: .leading).padding(12)
            .background(color.opacity(0.10), in: RoundedRectangle(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(color.opacity(0.25)))
    }
}

private struct UpdateNotice: View {
    @ObservedObject var updater: UpdateService
    var body: some View {
        if let version = updater.availableVersion {
            HStack {
                Text(updater.isBusy ? updater.status : "新版本 \(version) 可用").font(.system(size: 11))
                Spacer()
                Button("下载并重启更新") { updater.install() }.disabled(updater.isBusy).font(.system(size: 11))
            }.foregroundStyle(KrillTheme.mint).padding(12).background(KrillTheme.mint.opacity(0.1))
                .clipShape(RoundedRectangle(cornerRadius: 9)).padding(.horizontal, 24).padding(.top, 10)
        }
    }
}

private struct UpdateSettings: View {
    @ObservedObject var updater: UpdateService
    var body: some View {
        Card {
            Text("应用更新 · \(updater.currentVersion)").fontWeight(.semibold)
            Text(updater.status).font(.system(size: 12)).foregroundStyle(KrillTheme.muted)
            HStack(spacing: 16) {
                Button("检查更新") { updater.checkNow() }.disabled(updater.isBusy)
                if updater.availableVersion != nil {
                    Button("下载并重启更新") { updater.install() }.disabled(updater.isBusy).buttonStyle(.borderedProminent)
                }
            }
        }
    }
}

private enum SMSettings {
    @MainActor static func openLoginItems() {
        // The public ServiceManagement helper opens the correct Settings location.
        SMAppService.openSystemSettingsLoginItems()
    }
}
