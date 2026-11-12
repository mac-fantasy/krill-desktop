import KrillCore
import SwiftUI

struct UsageTrendChart: View {
    private let data: TrendChartData
    @State private var hoveredStart: Date?
    private let plotHeight: CGFloat = 104

    init(buckets: [TrendBucket]) { data = TrendChartData(buckets: buckets) }

    private var selectedIndex: Int? {
        data.buckets.firstIndex { $0.bucketStart == hoveredStart }
    }

    var body: some View {
        if data.buckets.isEmpty {
            Text("暂无时段统计").foregroundStyle(KrillTheme.muted)
                .frame(maxWidth: .infinity, minHeight: 104)
        } else {
            VStack(alignment: .leading, spacing: 8) {
                // Reserve the readout space so hovering never moves the plot under the pointer.
                Text(selectedIndex.map { data.tooltip(at: $0) } ?? "悬停图表查看时段、费用与请求数")
                    .font(.system(size: 11)).monospacedDigit()
                    .foregroundStyle(selectedIndex == nil ? KrillTheme.muted : KrillTheme.mint)
                    .lineLimit(2)
                    .frame(maxWidth: .infinity, minHeight: 32, maxHeight: 32, alignment: .leading)
                    .accessibilityIdentifier("usageTrendReadout")
                GeometryReader { geometry in
                    VStack(spacing: 0) {
                        plot(width: geometry.size.width)
                        timeAxis(width: geometry.size.width)
                    }
                }.frame(height: plotHeight + 26)
            }
            .onDisappear { hoveredStart = nil }
        }
    }

    private func plot(width: CGFloat) -> some View {
        let slotWidth = width / CGFloat(data.buckets.count)
        return ZStack(alignment: .bottomLeading) {
            ForEach(data.buckets.indices, id: \.self) { index in
                RoundedRectangle(cornerRadius: 2)
                    .fill(
                        data.buckets[index].totalCostUsd == nil
                            ? KrillTheme.muted : KrillTheme.mint.opacity(selectedIndex == index ? 1 : 0.75)
                    )
                    .frame(width: max(1, slotWidth - 4), height: data.barHeight(at: index))
                    .position(
                        x: data.centerX(at: index, width: width), y: plotHeight - data.barHeight(at: index) / 2
                    )
                    .accessibilityLabel(data.tooltip(at: index))
            }
            if let selectedIndex {
                Path { path in
                    let x = data.centerX(at: selectedIndex, width: width)
                    path.move(to: CGPoint(x: x, y: 0))
                    path.addLine(to: CGPoint(x: x, y: plotHeight))
                }.stroke(KrillTheme.mint.opacity(0.6), style: StrokeStyle(lineWidth: 1, dash: [3, 3]))
                    .accessibilityHidden(true)
            }
            Rectangle().fill(KrillTheme.border).frame(height: 1).accessibilityHidden(true)
        }
        .frame(width: width, height: plotHeight)
        .contentShape(Rectangle())
        .onContinuousHover { phase in
            switch phase {
            case .active(let location):
                hoveredStart = data.index(atX: location.x, width: width).map {
                    data.buckets[$0].bucketStart
                }
            case .ended:
                hoveredStart = nil
            }
        }
    }

    private func timeAxis(width: CGFloat) -> some View {
        ZStack(alignment: .topLeading) {
            ForEach(data.axisIndices, id: \.self) { index in
                let x = data.centerX(at: index, width: width)
                Rectangle().fill(KrillTheme.border).frame(width: 1, height: 4)
                    .position(x: x, y: 2).accessibilityHidden(true)
                Text(data.timeLabel(at: index)).font(.system(size: 9)).monospacedDigit()
                    .foregroundStyle(KrillTheme.muted)
                    .position(x: min(max(18, x), max(18, width - 18)), y: 15)
            }
        }.frame(width: width, height: 26)
    }
}

struct TrendChartData {
    let buckets: [TrendBucket]
    private let maximum: Decimal

    init(buckets: [TrendBucket]) {
        self.buckets = Array(buckets.sorted { $0.bucketStart < $1.bucketStart }.suffix(48))
        maximum = self.buckets.compactMap(\.totalCostUsd).max() ?? 0
    }

    var axisIndices: [Int] {
        let count = min(5, buckets.count)
        guard count > 1 else { return Array(buckets.indices) }
        return (0..<count).map {
            Int((Double($0) * Double(buckets.count - 1) / Double(count - 1)).rounded())
        }
    }

    func index(atX x: CGFloat, width: CGFloat) -> Int? {
        guard !buckets.isEmpty, width.isFinite, width > 0, x.isFinite, x >= 0, x <= width else {
            return nil
        }
        return min(buckets.count - 1, Int(x / width * CGFloat(buckets.count)))
    }

    func centerX(at index: Int, width: CGFloat) -> CGFloat {
        (CGFloat(index) + 0.5) * width / CGFloat(buckets.count)
    }

    func barHeight(at index: Int) -> CGFloat {
        guard maximum > 0, let cost = buckets[index].totalCostUsd, cost > 0 else { return 3 }
        let proportion = NSDecimalNumber(decimal: cost / maximum).doubleValue
        return CGFloat(max(3, min(88, proportion * 88)))
    }

    func timeLabel(at index: Int, timeZone: TimeZone = .current) -> String {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "zh_CN")
        formatter.timeZone = timeZone
        formatter.dateFormat = "HH:mm"
        return formatter.string(from: buckets[index].bucketStart)
    }

    func tooltip(at index: Int) -> String {
        let bucket = buckets[index]
        return
            "\(DisplayFormat.date(bucket.bucketStart)) · 费用 \(DisplayFormat.cost(bucket.totalCostUsd)) · \(bucket.requestCount) 次请求"
    }
}
