import Foundation

enum Mode: String {
    case light, dark

    var label: String { self == .dark ? "sombre" : "clair" }
}

/// The two switch times, in minutes since midnight, stored in the app's
/// defaults. The launchd agent runs this same binary, so both sides read the
/// same domain.
struct Schedule: Equatable {
    var lightAt: Int
    var darkAt: Int

    private static let defaults = UserDefaults.standard

    static var saved: Schedule {
        defaults.register(defaults: ["lightAt": 9 * 60, "darkAt": 18 * 60])
        return Schedule(lightAt: defaults.integer(forKey: "lightAt"),
                        darkAt: defaults.integer(forKey: "darkAt"))
    }

    func save() {
        Self.defaults.set(lightAt, forKey: "lightAt")
        Self.defaults.set(darkAt, forKey: "darkAt")
    }

    /// The mode the schedule asks for at `date`. The dark window runs from
    /// `darkAt` to `lightAt` and may wrap past midnight.
    func mode(at date: Date = Date()) -> Mode {
        let now = Self.minutes(of: date)
        let dark = darkAt < lightAt
            ? now >= darkAt && now < lightAt
            : now >= darkAt || now < lightAt
        return dark ? .dark : .light
    }

    /// The minute of the day at which the current mode ends.
    func nextSwitch(after date: Date = Date()) -> Int {
        mode(at: date) == .dark ? lightAt : darkAt
    }

    /// The mode last set by ClairObscur. The agent only acts when the schedule
    /// disagrees with it, i.e. when a switch time has passed since — so a
    /// manual change holds until the next switch, while a switch missed with
    /// the Mac off is still caught up at the next login.
    static var lastApplied: Mode? {
        get { defaults.string(forKey: "lastApplied").flatMap(Mode.init) }
        set { defaults.set(newValue?.rawValue, forKey: "lastApplied") }
    }

    static func minutes(of date: Date) -> Int {
        let c = Calendar.current.dateComponents([.hour, .minute], from: date)
        return (c.hour ?? 0) * 60 + (c.minute ?? 0)
    }

    static func format(_ minutes: Int) -> String {
        String(format: "%d:%02d", minutes / 60, minutes % 60)
    }
}
