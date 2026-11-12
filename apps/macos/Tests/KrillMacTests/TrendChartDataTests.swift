import Foundation
import KrillCore
import XCTest

@testable import KrillMac

final class TrendChartDataTests: XCTestCase {
    private let start = Date(timeIntervalSince1970: 1_800_000_000)

    private func buckets(count: Int) -> [TrendBucket] {
        (0..<count).map {
            TrendBucket(
                start: start.addingTimeInterval(Double($0) * 3600), requests: Int64($0), cost: Decimal($0))
        }
    }

    func testUnorderedDataKeepsLatest48InTimeOrder() {
        let source = buckets(count: 60)
        let data = TrendChartData(buckets: source.reversed())
        XCTAssertEqual(data.buckets.count, 48)
        XCTAssertEqual(data.buckets.first?.bucketStart, source[12].bucketStart)
        XCTAssertEqual(data.buckets.last?.bucketStart, source[59].bucketStart)
    }

    func testTimeTicksIncludeBothEndsWithoutCrowding() {
        for count in [0, 1, 2, 4, 5, 24, 48] {
            let data = TrendChartData(buckets: buckets(count: count))
            XCTAssertEqual(data.axisIndices.count, min(count, 5))
            XCTAssertEqual(Set(data.axisIndices).count, data.axisIndices.count)
            XCTAssertEqual(data.axisIndices, data.axisIndices.sorted())
            if count > 0 {
                XCTAssertEqual(data.axisIndices.first, 0)
                XCTAssertEqual(data.axisIndices.last, count - 1)
            }
        }
    }

    func testHoverCoversEntireSlotsIncludingGapsAndEdges() {
        let data = TrendChartData(buckets: buckets(count: 24))
        XCTAssertEqual(data.index(atX: 0, width: 480), 0)
        XCTAssertEqual(data.index(atX: 19.9, width: 480), 0)
        XCTAssertEqual(data.index(atX: 20, width: 480), 1)
        XCTAssertEqual(data.index(atX: 480, width: 480), 23)
        for index in data.buckets.indices {
            let center = data.centerX(at: index, width: 480)
            XCTAssertEqual(center, CGFloat(index * 20 + 10))
            XCTAssertEqual(data.index(atX: center, width: 480), index)
        }
    }

    func testInvalidHoverCoordinatesAndEmptyDataHaveNoSelection() {
        let data = TrendChartData(buckets: buckets(count: 1))
        for x in [CGFloat(-1), 481, .nan, .infinity, -.infinity] {
            XCTAssertNil(data.index(atX: x, width: 480))
        }
        for width in [CGFloat(0), -1, .nan, .infinity] {
            XCTAssertNil(data.index(atX: 0, width: width))
        }
        XCTAssertNil(TrendChartData(buckets: []).index(atX: 0, width: 480))
        XCTAssertEqual(data.index(atX: 240, width: 480), 0)
    }

    func testZeroNegativeAndUnknownCostsRemainHoverable() {
        var source = buckets(count: 4)
        source[0].totalCostUsd = nil
        source[1].totalCostUsd = 0
        source[2].totalCostUsd = -1
        source[3].totalCostUsd = 2
        let data = TrendChartData(buckets: source)
        XCTAssertEqual((0..<4).map { data.barHeight(at: $0) }, [3, 3, 3, 88])
        XCTAssertTrue(data.tooltip(at: 0).contains("费用 —"))
        XCTAssertTrue(data.tooltip(at: 1).contains("费用 $0.00"))
        let zeroOnly = TrendChartData(buckets: Array(source.prefix(3)))
        XCTAssertEqual((0..<3).map { zeroOnly.barHeight(at: $0) }, [3, 3, 3])
    }

    func testTooltipPreservesSmallCostsAndExactRequestCount() throws {
        let source = TrendBucket(
            start: start, requests: 12_345, cost: try XCTUnwrap(Decimal(string: "0.0007")))
        let data = TrendChartData(buckets: [source])
        XCTAssertEqual(data.tooltip(at: 0), "\(DisplayFormat.date(start)) · 费用 $0.0007 · 12345 次请求")
    }

    func testTimeTicksUseLocalTimeAcrossMidnightAndDST() throws {
        let formatter = ISO8601DateFormatter()
        let utc = try XCTUnwrap(TimeZone(secondsFromGMT: 0))
        let midnight = try XCTUnwrap(formatter.date(from: "2026-10-15T23:00:00Z"))
        let data = TrendChartData(buckets: [
            TrendBucket(start: midnight, requests: 1, cost: 1),
            TrendBucket(start: midnight.addingTimeInterval(3600), requests: 1, cost: 1),
        ])
        XCTAssertEqual(data.timeLabel(at: 0, timeZone: utc), "23:00")
        XCTAssertEqual(data.timeLabel(at: 1, timeZone: utc), "00:00")
        let losAngeles = try XCTUnwrap(TimeZone(identifier: "America/Los_Angeles"))
        let transition = try XCTUnwrap(formatter.date(from: "2026-03-08T09:00:00Z"))
        let dstData = TrendChartData(buckets: [
            TrendBucket(start: transition, requests: 1, cost: 1),
            TrendBucket(start: transition.addingTimeInterval(3600), requests: 1, cost: 1),
        ])
        XCTAssertEqual(dstData.timeLabel(at: 0, timeZone: losAngeles), "01:00")
        XCTAssertEqual(dstData.timeLabel(at: 1, timeZone: losAngeles), "03:00")
    }
}
