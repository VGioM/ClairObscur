import AppKit
import SwiftUI

@MainActor
final class Model: ObservableObject {
    @Published var light: Date { didSet { scheduleSave() } }
    @Published var dark: Date { didSet { scheduleSave() } }
    @Published private(set) var installed = Agent.isInstalled
    @Published var error: String?
    @Published private(set) var now = Date()

    private var saveTask: Task<Void, Never>?
    private var timer: Timer?

    init() {
        let saved = Schedule.saved
        light = Self.date(saved.lightAt)
        dark = Self.date(saved.darkAt)
        // Keeps « jusqu'à … » and the warnings current while the window is open.
        timer = Timer.scheduledTimer(withTimeInterval: 20, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.now = Date() }
        }
    }

    var schedule: Schedule {
        Schedule(lightAt: Schedule.minutes(of: light), darkAt: Schedule.minutes(of: dark))
    }

    var sameTimes: Bool { schedule.lightAt == schedule.darkAt }

    func setInstalled(_ on: Bool) {
        error = nil
        do {
            if on {
                guard !sameTimes else { return }
                schedule.save()
                // Done here rather than left to the agent's first run, so the
                // Automation prompt shows up in front of the window.
                error = Applier.apply(schedule.mode())
                try Agent.install(schedule)
            } else {
                try Agent.uninstall()
            }
        } catch {
            self.error = error.localizedDescription
        }
        installed = Agent.isInstalled
    }

    func applyNow() {
        error = Applier.apply(schedule.mode())
        now = Date()
    }

    func reinstall() { setInstalled(true) }

    // The pickers report every step; wait for them to settle before touching launchd.
    private func scheduleSave() {
        saveTask?.cancel()
        saveTask = Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(700))
            guard !Task.isCancelled else { return }
            save()
        }
    }

    private func save() {
        let schedule = self.schedule
        guard !sameTimes, schedule != Schedule.saved else { return }
        schedule.save()
        guard installed else { return }
        error = nil
        do {
            try Agent.install(schedule)
            Applier.runScheduled()
        } catch {
            self.error = error.localizedDescription
        }
    }

    private static func date(_ minutes: Int) -> Date {
        Calendar.current.date(bySettingHour: minutes / 60, minute: minutes % 60,
                              second: 0, of: Date()) ?? Date()
    }
}

struct ContentView: View {
    @StateObject private var model = Model()

    var body: some View {
        VStack(spacing: 0) {
            header
            Form {
                Section("Horaires") {
                    DatePicker("Mode clair à", selection: $model.light, displayedComponents: .hourAndMinute)
                    DatePicker("Mode sombre à", selection: $model.dark, displayedComponents: .hourAndMinute)
                    if model.sameTimes {
                        warning("Les deux heures doivent être différentes.")
                    }
                }

                Section {
                    Toggle(isOn: Binding(get: { model.installed }, set: { model.setInstalled($0) })) {
                        Text("Changer automatiquement")
                        Text(model.installed
                             ? "Actif, y compris après un redémarrage ou une mise en veille."
                             : "Inactif. Aucun agent launchd n'est installé.")
                    }
                    .disabled(model.sameTimes && !model.installed)

                    if model.installed && !Agent.pointsToThisCopy {
                        HStack {
                            warning("L'agent lance une autre copie de ClairObscur.")
                            Spacer()
                            Button("Utiliser celle-ci") { model.reinstall() }
                        }
                    }
                    if Appearance.switchesAutomatically {
                        HStack {
                            warning("L'apparence de macOS est sur « Auto » et va contredire ces horaires.")
                            Spacer()
                            Button("Ouvrir") { open("x-apple.systempreferences:com.apple.Appearance-Settings.extension") }
                        }
                    }
                }

                if let error = model.error {
                    Section {
                        Label(error, systemImage: "xmark.octagon.fill")
                            .foregroundStyle(.red)
                        if error.contains("Automatisation") {
                            Button("Ouvrir les réglages d'Automatisation") {
                                open("x-apple.systempreferences:com.apple.preference.security?Privacy_Automation")
                            }
                        }
                    }
                }
            }
            .formStyle(.grouped)
            .scrollDisabled(true)

            footer
        }
        .frame(width: 420)
        .fixedSize(horizontal: false, vertical: true)
    }

    private var header: some View {
        let mode = model.schedule.mode(at: model.now)
        return HStack(spacing: 14) {
            Image(systemName: mode == .dark ? "moon.stars.fill" : "sun.max.fill")
                .font(.system(size: 26, weight: .semibold))
                .foregroundStyle(mode == .dark ? Color.indigo : Color.orange)
                .frame(width: 52, height: 52)
                .background(.quaternary, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                .contentTransition(.symbolEffect(.replace))
            VStack(alignment: .leading, spacing: 2) {
                Text("ClairObscur").font(.title2.bold())
                Text(model.sameTimes
                     ? "Choisissez deux heures différentes."
                     : "Mode \(mode.label) prévu jusqu'à \(Schedule.format(model.schedule.nextSwitch(after: model.now)))")
                    .foregroundStyle(.secondary)
            }
            Spacer()
        }
        .padding(.horizontal, 20)
        .padding(.top, 18)
    }

    private var footer: some View {
        HStack {
            if !Bundle.main.bundlePath.hasPrefix("/Applications/") {
                Text("Placez l'app dans Applications avant d'activer : l'agent retient son emplacement.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer()
            Button("Appliquer maintenant") { model.applyNow() }
                .keyboardShortcut(.defaultAction)
                .disabled(model.sameTimes)
        }
        .padding(.horizontal, 20)
        .padding(.bottom, 18)
    }

    private func warning(_ text: String) -> some View {
        Label(text, systemImage: "exclamationmark.triangle.fill")
            .foregroundStyle(.orange)
            .fixedSize(horizontal: false, vertical: true)
    }

    private func open(_ url: String) {
        if let url = URL(string: url) { NSWorkspace.shared.open(url) }
    }
}
