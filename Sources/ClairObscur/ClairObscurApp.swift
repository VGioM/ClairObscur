import AppKit
import SwiftUI

@main
enum Entry {
    static func main() {
        // `--apply` is how the launchd agent runs us: apply and quit, no UI.
        if CommandLine.arguments.contains("--apply") {
            Applier.runScheduled()
            return
        }
        ClairObscurApp.main()
    }
}

struct ClairObscurApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate

    var body: some Scene {
        Window("ClairObscur", id: "main") {
            ContentView()
        }
        .windowResizability(.contentSize)
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}
