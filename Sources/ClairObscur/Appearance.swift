import Foundation

/// Reads and sets the system-wide appearance.
enum Appearance {
    static var isDark: Bool {
        CFPreferencesAppSynchronize(kCFPreferencesAnyApplication)
        return CFPreferencesCopyAppValue("AppleInterfaceStyle" as CFString,
                                         kCFPreferencesAnyApplication) as? String == "Dark"
    }

    /// True when Réglages > Apparence is on « Auto », which would fight us.
    static var switchesAutomatically: Bool {
        CFPreferencesAppSynchronize(kCFPreferencesAnyApplication)
        return CFPreferencesCopyAppValue("AppleInterfaceStyleSwitchesAutomatically" as CFString,
                                         kCFPreferencesAnyApplication) as? Bool ?? false
    }

    /// Goes through System Events, the public route; the first call asks for
    /// the Automation permission. Returns an error message on failure.
    static func set(_ mode: Mode) -> String? {
        let source = """
            tell application "System Events" to tell appearance preferences \
            to set dark mode to \(mode == .dark)
            """
        var error: NSDictionary?
        NSAppleScript(source: source)?.executeAndReturnError(&error)
        guard let error else { return nil }
        if error[NSAppleScript.errorNumber] as? Int == -1743 {
            return "ClairObscur n'a pas le droit de piloter System Events. "
                + "Autorisez-le dans Réglages Système › Confidentialité et sécurité › Automatisation."
        }
        return error[NSAppleScript.errorMessage] as? String ?? "\(error)"
    }
}

/// Applies the schedule, from the agent or from the window.
enum Applier {
    /// What the agent runs at login, at each switch time, on wake and every
    /// few minutes. Does nothing unless a switch time has passed since the
    /// last change, so it never overrides a manual choice in between.
    static func runScheduled() {
        let target = Schedule.saved.mode()
        guard Schedule.lastApplied != target else { return }
        apply(target)
    }

    @discardableResult
    static func apply(_ mode: Mode) -> String? {
        if Appearance.isDark != (mode == .dark) {
            if let error = Appearance.set(mode) {
                Log.write("échec du passage en mode \(mode.label) : \(error)")
                return error
            }
            Log.write("passage en mode \(mode.label)")
        }
        Schedule.lastApplied = mode
        return nil
    }
}

enum Log {
    static let url = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent("Library/Logs/ClairObscur.log")

    static func write(_ message: String) {
        let stamp = Date().formatted(date: .numeric, time: .standard)
        let line = Data("[\(stamp)] \(message)\n".utf8)
        if let handle = try? FileHandle(forWritingTo: url) {
            handle.seekToEndOfFile()
            handle.write(line)
            try? handle.close()
        } else {
            try? line.write(to: url)
        }
    }
}
