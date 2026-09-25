import Foundation

/// Date-picker presentation only; the Rust engine resolves daily seeds.
public enum DailyRunDate {
    public static var calendar: Calendar {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        return calendar
    }
    private static var formatter: DateFormatter {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.calendar = calendar
        formatter.timeZone = calendar.timeZone
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.isLenient = false
        return formatter
    }
    public static func code(_ date: Date = Date()) -> String { formatter.string(from: date) }
    public static func date(_ code: String) -> Date? { formatter.date(from: code) }
}
