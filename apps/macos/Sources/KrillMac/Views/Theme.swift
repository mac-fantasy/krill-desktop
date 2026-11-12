import SwiftUI

enum KrillTheme {
    static let background = Color(red: 0.055, green: 0.090, blue: 0.115)
    static let card = Color(red: 0.083, green: 0.133, blue: 0.160)
    static let mint = Color(red: 0.486, green: 0.890, blue: 0.804)
    static let amber = Color(red: 0.953, green: 0.737, blue: 0.451)
    static let muted = Color(red: 0.56, green: 0.66, blue: 0.71)
    static let border = Color.white.opacity(0.085)
}

extension ColorStyle {
    var color: Color {
        switch self { case .mint: KrillTheme.mint; case .amber: KrillTheme.amber; case .muted: KrillTheme.muted }
    }
}

struct GlassSurface: ViewModifier {
    var enabled: Bool
    var radius: CGFloat = 12
    @ViewBuilder func body(content: Content) -> some View {
        #if compiler(>=6.2)
        if #available(macOS 26.0, *), enabled {
            content.glassEffect(.regular, in: RoundedRectangle(cornerRadius: radius))
        } else {
            content.background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: radius))
        }
        #else
        content.background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: radius))
        #endif
    }
}

struct Card<Content: View>: View {
    @ViewBuilder var content: Content
    var body: some View {
        VStack(alignment: .leading, spacing: 14) { content }
            .frame(maxWidth: .infinity, alignment: .leading).padding(18)
            .background(KrillTheme.card, in: RoundedRectangle(cornerRadius: 15))
            .overlay(RoundedRectangle(cornerRadius: 15).strokeBorder(KrillTheme.border))
    }
}

struct QuotaRing: View {
    var percent: Double?
    var color: Color = KrillTheme.mint
    var size: CGFloat = 96
    var body: some View {
        ZStack {
            Circle().stroke(KrillTheme.border, lineWidth: 5)
            Circle().trim(from: 0, to: CGFloat((percent ?? 0) / 100))
                .stroke(color, style: StrokeStyle(lineWidth: 5, lineCap: .round)).rotationEffect(.degrees(-90))
            VStack(spacing: 4) {
                Text(percent.map { String(format: "%.0f%%", $0) } ?? "—").font(.system(size: 23, weight: .semibold, design: .rounded)).monospacedDigit()
                Text("剩余").font(.system(size: 10)).foregroundStyle(KrillTheme.muted)
            }
        }.frame(width: size, height: size)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("剩余额度").accessibilityValue(percent.map { String(format: "%.0f%%", $0) } ?? "未知")
    }
}

struct Metric: View {
    var title: String
    var value: String
    var accent = false
    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            Text(title).font(.system(size: 11)).foregroundStyle(KrillTheme.muted)
            Text(value).font(.system(size: 23, weight: .semibold, design: .rounded)).monospacedDigit()
                .foregroundStyle(accent ? KrillTheme.mint : .white).lineLimit(1).minimumScaleFactor(0.65)
        }.frame(maxWidth: .infinity, alignment: .leading).padding(15)
            .background(KrillTheme.card, in: RoundedRectangle(cornerRadius: 13))
            .overlay(RoundedRectangle(cornerRadius: 13).strokeBorder(KrillTheme.border))
    }
}
