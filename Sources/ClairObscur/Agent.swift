import Foundation

/// The per-user launchd agent that runs `ClairObscur --apply`.
///
/// It fires at login (RunAtLoad, which catches up a switch missed while the
/// Mac was off) and at both switch times (launchd runs a calendar job missed
/// during sleep as soon as the Mac wakes). Each run is a no-op unless a switch
/// is due — see `Applier.runScheduled`.
enum Agent {
    static let label = "com.clairobscur.agent"

    static let plistURL = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent("Library/LaunchAgents/\(label).plist")

    private static var domain: String { "gui/\(getuid())" }

    static var isInstalled: Bool {
        FileManager.default.fileExists(atPath: plistURL.path)
    }

    /// The binary the installed agent launches, to spot a moved app.
    static var installedExecutable: String? {
        guard let data = try? Data(contentsOf: plistURL),
              let plist = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any],
              let args = plist["ProgramArguments"] as? [String] else { return nil }
        return args.first
    }

    static var pointsToThisCopy: Bool {
        installedExecutable == Bundle.main.executablePath
    }

    /// Writes the plist for `schedule` and (re)loads it.
    static func install(_ schedule: Schedule) throws {
        guard let executable = Bundle.main.executablePath else {
            throw AgentError("Impossible de trouver l'exécutable de l'app.")
        }
        let plist: [String: Any] = [
            "Label": label,
            "ProgramArguments": [executable, "--apply"],
            "RunAtLoad": true,
            "StartCalendarInterval": [schedule.lightAt, schedule.darkAt].map {
                ["Hour": $0 / 60, "Minute": $0 % 60]
            },
            "StandardErrorPath": Log.url.path,
            "ProcessType": "Background",
        ]
        let data = try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0)
        try FileManager.default.createDirectory(at: plistURL.deletingLastPathComponent(),
                                                withIntermediateDirectories: true)
        try data.write(to: plistURL, options: .atomic)

        launchctl("bootout", "\(domain)/\(label)")
        let (status, output) = launchctl("bootstrap", domain, plistURL.path)
        if status != 0 {
            throw AgentError("launchctl a refusé l'agent (\(status)) : \(output)")
        }
        Log.write("agent installé — clair à \(Schedule.format(schedule.lightAt)), "
                  + "sombre à \(Schedule.format(schedule.darkAt))")
    }

    static func uninstall() throws {
        launchctl("bootout", "\(domain)/\(label)")
        if isInstalled {
            try FileManager.default.removeItem(at: plistURL)
        }
        Log.write("agent supprimé")
    }

    @discardableResult
    private static func launchctl(_ args: String...) -> (Int32, String) {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/launchctl")
        process.arguments = args
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = pipe
        do { try process.run() } catch { return (-1, error.localizedDescription) }
        process.waitUntilExit()
        let output = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
        return (process.terminationStatus, output.trimmingCharacters(in: .whitespacesAndNewlines))
    }
}

struct AgentError: LocalizedError {
    let errorDescription: String?
    init(_ message: String) { errorDescription = message }
}
