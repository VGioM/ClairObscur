//! ClairObscur : choisit à quelle heure KDE Plasma passe du thème global clair
//! au thème sombre (et inversement), comme l'appli du même nom sur Mac.
//!
//! Plasma sait déjà basculer tout seul (kdeglobals, [KDE] AutomaticLookAndFeel)
//! en suivant l'horaire du démon knighttimed. Cette appli ne fait que régler
//! cet horaire :
//! - ~/.config/knighttimerc, [General] Source = Automatic (soleil) ou Times ;
//!   [Times] SunriseStart / SunsetStart au format HH:mm, TransitionDuration en
//!   secondes. knighttimed ne relit pas ce fichier tout seul : on le redémarre.
//! - ~/.config/kdeglobals, [KDE] AutomaticLookAndFeel, DefaultLightLookAndFeel,
//!   DefaultDarkLookAndFeel (écrits avec kwriteconfig6 --notify).

use chrono::{DateTime, Local, NaiveTime, TimeZone};
use eframe::egui::{self, Color32, CornerRadius, Margin, RichText, Stroke};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// Durée de la transition écrite en mode « heures choisies ». Plasma change de
/// thème au milieu de la transition : avec 10 s, à HH:MM:05.
const TRANSITION_S: u32 = 10;

// ---------------------------------------------------------------------------
// Fichiers de configuration KDE

fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"))
}

/// Lit un fichier KConfig : (groupe, clé) → valeur. Les groupes imbriqués
/// gardent leur nom complet (« Colors][Button »).
fn parse_ini(text: &str) -> HashMap<(String, String), String> {
    let mut out = HashMap::new();
    let mut group = String::new();
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') { continue; }
        if l.starts_with('[') && l.ends_with(']') { group = l[1..l.len() - 1].to_string(); continue; }
        if let Some((k, v)) = l.split_once('=') {
            let k = k.trim();
            if k.contains('[') { continue; } // clés traduites (Name[fr])
            out.insert((group.clone(), k.to_string()), v.trim().to_string());
        }
    }
    out
}

fn read_ini(name: &str) -> HashMap<(String, String), String> {
    parse_ini(&std::fs::read_to_string(config_dir().join(name)).unwrap_or_default())
}

fn get<'a>(ini: &'a HashMap<(String, String), String>, g: &str, k: &str) -> Option<&'a str> {
    ini.get(&(g.to_string(), k.to_string())).map(|s| s.as_str())
}

fn parse_hhmm(s: &str) -> Option<(u32, u32)> {
    let t = NaiveTime::parse_from_str(s.trim(), "%H:%M").or_else(|_| NaiveTime::parse_from_str(s.trim(), "%H:%M:%S")).ok()?;
    use chrono::Timelike;
    Some((t.hour(), t.minute()))
}

fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(cmd).args(args).output().map_err(|e| format!("{cmd} : {e}"))?;
    if out.status.success() { Ok(String::from_utf8_lossy(&out.stdout).into_owned()) }
    else { Err(format!("{cmd} : {}", String::from_utf8_lossy(&out.stderr).trim())) }
}

fn kwrite(file: &str, group: &str, key: &str, value: &str) -> Result<(), String> {
    run("kwriteconfig6", &["--file", file, "--group", group, "--key", key, value, "--notify"]).map(|_| ())
}

// ---------------------------------------------------------------------------
// Thèmes globaux installés

#[derive(Clone, Debug, PartialEq)]
struct Theme { id: String, name: String }

fn data_dirs() -> Vec<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let mut v = vec![std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"))];
    let sys = std::env::var("XDG_DATA_DIRS").unwrap_or_default();
    let sys = if sys.is_empty() { "/usr/local/share:/usr/share".to_string() } else { sys };
    v.extend(sys.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    v
}

fn theme_name(dir: &Path) -> Option<String> {
    let j: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("metadata.json")).ok()?).ok()?;
    let k = j.get("KPlugin")?;
    k.get("Name[fr]").or_else(|| k.get("Name")).and_then(|n| n.as_str()).map(|s| s.trim().to_string())
}

fn list_themes() -> Vec<Theme> {
    let mut out: Vec<Theme> = Vec::new();
    for d in data_dirs() {
        let Ok(rd) = std::fs::read_dir(d.join("plasma/look-and-feel")) else { continue };
        for e in rd.flatten() {
            let id = e.file_name().to_string_lossy().into_owned();
            if out.iter().any(|t| t.id == id) { continue; }
            if let Some(name) = theme_name(&e.path()) { out.push(Theme { id, name }); }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

// ---------------------------------------------------------------------------
// Réglages

#[derive(Clone, Debug, PartialEq)]
struct Settings {
    auto: bool,
    sun: bool,
    light_at: (u32, u32),
    dark_at: (u32, u32),
    light_theme: String,
    dark_theme: String,
}

fn read_settings() -> (Settings, String) {
    let kg = read_ini("kdeglobals");
    let kn = read_ini("knighttimerc");
    let s = Settings {
        auto: get(&kg, "KDE", "AutomaticLookAndFeel") == Some("true"),
        sun: get(&kn, "General", "Source") != Some("Times"),
        light_at: get(&kn, "Times", "SunriseStart").and_then(parse_hhmm).unwrap_or((7, 0)),
        dark_at: get(&kn, "Times", "SunsetStart").and_then(parse_hhmm).unwrap_or((19, 0)),
        light_theme: get(&kg, "KDE", "DefaultLightLookAndFeel").unwrap_or("org.kde.breeze.desktop").to_string(),
        dark_theme: get(&kg, "KDE", "DefaultDarkLookAndFeel").unwrap_or("org.kde.breezedark.desktop").to_string(),
    };
    let current = get(&kg, "KDE", "LookAndFeelPackage").unwrap_or("").to_string();
    (s, current)
}

fn save_settings(new: &Settings, old: &Settings) -> Result<(), String> {
    if new.light_theme != old.light_theme { kwrite("kdeglobals", "KDE", "DefaultLightLookAndFeel", &new.light_theme)?; }
    if new.dark_theme != old.dark_theme { kwrite("kdeglobals", "KDE", "DefaultDarkLookAndFeel", &new.dark_theme)?; }
    let schedule_changed = new.sun != old.sun || (!new.sun && (new.light_at != old.light_at || new.dark_at != old.dark_at));
    if schedule_changed {
        let hm = |(h, m): (u32, u32)| format!("{h:02}:{m:02}");
        kwrite("knighttimerc", "General", "Source", if new.sun { "Automatic" } else { "Times" })?;
        if !new.sun {
            kwrite("knighttimerc", "Times", "SunriseStart", &hm(new.light_at))?;
            kwrite("knighttimerc", "Times", "SunsetStart", &hm(new.dark_at))?;
            kwrite("knighttimerc", "Times", "TransitionDuration", &TRANSITION_S.to_string())?;
        }
        run("systemctl", &["--user", "restart", "plasma-knighttimed.service"])?;
    }
    // En dernier : Plasma rebascule tout de suite sur le thème de l'horaire.
    if new.auto != old.auto || schedule_changed {
        kwrite("kdeglobals", "KDE", "AutomaticLookAndFeel", if new.auto { "true" } else { "false" })?;
    }
    Ok(())
}

/// Applique un thème tout de suite. plasma-apply-lookandfeel coupe le
/// basculement automatique : on le remet si besoin.
fn apply_now(theme: &str, keep_auto: bool) -> Result<(), String> {
    run("plasma-apply-lookandfeel", &["-a", theme])?;
    if keep_auto { kwrite("kdeglobals", "KDE", "AutomaticLookAndFeel", "true")?; }
    Ok(())
}

// ---------------------------------------------------------------------------
// Horaire calculé par knighttimed (D-Bus org.kde.NightTime)

#[derive(Clone, Copy, Debug)]
struct Cycle { morning: (i64, i64), evening: (i64, i64) }

impl Cycle {
    /// Instants où Plasma change de thème : au milieu de chaque transition.
    fn light_at(&self) -> i64 { (self.morning.0 + self.morning.1) / 2 }
    fn dark_at(&self) -> i64 { (self.evening.0 + self.evening.1) / 2 }
}

fn parse_schedule(json: &str) -> Option<(u32, Vec<Cycle>)> {
    let j: serde_json::Value = serde_json::from_str(json).ok()?;
    let d = j.get("data")?.get(0)?;
    let cookie = d.pointer("/Cookie/data")?.as_u64()? as u32;
    let cycles = d.pointer("/Schedule/data/1/data")?.as_array()?;
    let v = cycles.iter().filter_map(|c| {
        let n: Vec<i64> = c.as_array()?.iter().filter_map(|x| x.as_i64()).collect();
        (n.len() == 5).then(|| Cycle { morning: (n[1], n[2]), evening: (n[3], n[4]) })
    }).collect();
    Some((cookie, v))
}

fn fetch_schedule() -> Vec<Cycle> {
    let dest = ["--user", "--json=short", "call", "org.kde.NightTime", "/org/kde/NightTime/Manager", "org.kde.NightTime.Manager"];
    let mut a = dest.to_vec();
    a.extend(["Subscribe", "a{sv}", "0"]);
    let Ok(out) = run("busctl", &a) else { return Vec::new() };
    let Some((cookie, cycles)) = parse_schedule(&out) else { return Vec::new() };
    let c = cookie.to_string();
    let mut a = dest.to_vec();
    a.extend(["Unsubscribe", "u", &c]);
    let _ = run("busctl", &a);
    cycles
}

fn local(ms: i64) -> DateTime<Local> {
    Local.timestamp_millis_opt(ms).single().unwrap_or_else(Local::now)
}

/// Prochain passage après `now` : (vers le sombre ?, instant en ms).
fn next_switch(cycles: &[Cycle], now: i64) -> Option<(bool, i64)> {
    let mut all: Vec<(bool, i64)> = cycles.iter().flat_map(|c| [(false, c.light_at()), (true, c.dark_at())]).collect();
    all.sort_by_key(|x| x.1);
    all.into_iter().find(|x| x.1 > now)
}

/// Thème que l'horaire impose à `now` : sombre si le dernier passage était vers le sombre.
fn expected_dark(cycles: &[Cycle], now: i64) -> Option<bool> {
    let mut all: Vec<(bool, i64)> = cycles.iter().flat_map(|c| [(false, c.light_at()), (true, c.dark_at())]).collect();
    all.sort_by_key(|x| x.1);
    all.into_iter().filter(|x| x.1 <= now).last().map(|x| x.0)
}

/// Horaire du jour de `now` (lever, coucher), en ms.
fn today(cycles: &[Cycle], now: DateTime<Local>) -> Option<(i64, i64)> {
    cycles.iter().find(|c| local(c.light_at()).date_naive() == now.date_naive()).map(|c| (c.light_at(), c.dark_at()))
}

// ---------------------------------------------------------------------------
// Apparence

#[derive(Clone, Copy)]
struct Pal { bg: Color32, card: Color32, card2: Color32, line: Color32, text: Color32, muted: Color32, accent: Color32, on_accent: Color32, sun: Color32, moon: Color32 }

const DARK: Pal = Pal {
    bg: Color32::from_rgb(0x17, 0x18, 0x1B), card: Color32::from_rgb(0x21, 0x22, 0x26), card2: Color32::from_rgb(0x2A, 0x2C, 0x31),
    line: Color32::from_rgb(0x33, 0x35, 0x3B), text: Color32::from_rgb(0xF2, 0xF2, 0xF4), muted: Color32::from_rgb(0xA0, 0xA3, 0xAB),
    accent: Color32::from_rgb(0xF5, 0xA6, 0x3A), on_accent: Color32::from_rgb(0x22, 0x16, 0x05),
    sun: Color32::from_rgb(0xFF, 0xC4, 0x5A), moon: Color32::from_rgb(0xB9, 0xC3, 0xE8),
};

fn style(ctx: &egui::Context, p: &Pal) {
    let mut s = (*ctx.style()).clone();
    s.visuals = egui::Visuals::dark();
    let v = &mut s.visuals;
    v.panel_fill = p.bg;
    v.window_fill = p.card;
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = p.accent;
    v.selection.stroke = Stroke::new(1.0_f32, p.on_accent);
    v.extreme_bg_color = p.card2;
    v.clip_rect_margin = 0.0;
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(8);
    }
    v.widgets.inactive.bg_fill = p.card2;
    v.widgets.inactive.weak_bg_fill = p.card2;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.line);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.accent);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.line);
    s.spacing.item_spacing = egui::vec2(10.0, 8.0);
    s.spacing.button_padding = egui::vec2(14.0, 8.0);
    s.spacing.interact_size.y = 28.0;
    for (ts, size) in [(egui::TextStyle::Body, 15.0), (egui::TextStyle::Button, 15.0), (egui::TextStyle::Small, 12.5)] {
        if let Some(f) = s.text_styles.get_mut(&ts) { f.size = size; }
    }
    ctx.set_style(s);
}

fn card<R>(ui: &mut egui::Ui, p: &Pal, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new().fill(p.card).stroke(Stroke::new(1.0_f32, p.line)).corner_radius(CornerRadius::same(14)).inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title.to_uppercase()).size(12.0).color(p.muted).strong());
            ui.add_space(4.0);
            add(ui)
        }).inner
}

fn button(ui: &mut egui::Ui, p: &Pal, enabled: bool, text: &str, primary: bool, width: f32) -> bool {
    let rt = RichText::new(text).strong().color(if primary { p.on_accent } else { p.text });
    let mut b = egui::Button::new(rt).min_size(egui::vec2(width, 38.0)).corner_radius(CornerRadius::same(10));
    if primary { b = b.fill(p.accent); }
    ui.add_enabled(enabled, b).clicked()
}

/// Sélecteur d'heure HH : MM.
fn time_edit(ui: &mut egui::Ui, t: &mut (u32, u32)) {
    let fmt = |v: f64, _| format!("{:02}", v as u32);
    ui.add(egui::DragValue::new(&mut t.0).range(0..=23).speed(0.08).custom_formatter(fmt));
    ui.label(RichText::new(":").strong());
    ui.add(egui::DragValue::new(&mut t.1).range(0..=59).speed(0.15).custom_formatter(fmt));
}

fn sun_icon(ui: &mut egui::Ui, c: Color32) {
    let (r, painter) = ui.allocate_painter(egui::vec2(22.0, 22.0), egui::Sense::hover());
    let o = r.rect.center();
    painter.circle_filled(o, 5.0, c);
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        let d = egui::vec2(a.cos(), a.sin());
        painter.line_segment([o + d * 7.5, o + d * 10.0], Stroke::new(2.0_f32, c));
    }
}

fn moon_icon(ui: &mut egui::Ui, c: Color32, bg: Color32) {
    let (r, painter) = ui.allocate_painter(egui::vec2(22.0, 22.0), egui::Sense::hover());
    let o = r.rect.center();
    painter.circle_filled(o, 8.0, c);
    painter.circle_filled(o + egui::vec2(4.5, -3.5), 7.0, bg);
}

// ---------------------------------------------------------------------------
// Application

struct App {
    themes: Vec<Theme>,
    saved: Settings,
    edit: Settings,
    current: String,
    cycles: Vec<Cycle>,
    refreshed: Instant,
    msg: Option<(bool, String)>,
}

impl App {
    fn new() -> Self {
        let (s, current) = read_settings();
        App { themes: list_themes(), saved: s.clone(), edit: s, current, cycles: fetch_schedule(), refreshed: Instant::now(), msg: None }
    }

    /// Relit l'état de Plasma (le thème peut changer pendant que l'appli est ouverte).
    fn refresh(&mut self, schedule: bool) {
        let (s, current) = read_settings();
        if self.edit == self.saved { self.edit = s.clone(); }
        self.saved = s;
        self.current = current;
        if schedule { self.cycles = fetch_schedule(); }
        self.refreshed = Instant::now();
    }

    /// Après un enregistrement : si l'heure du passage est déjà dépassée
    /// (sombre réglé à 17:00 alors qu'il est 18:00), applique tout de suite le
    /// thème de l'horaire. Plasma le fait d'habitude seul ; ceci le garantit.
    fn enforce(&mut self) -> Result<(), String> {
        if !self.saved.auto { return Ok(()); }
        let Some(dark) = expected_dark(&self.cycles, Local::now().timestamp_millis()) else { return Ok(()) };
        let want = if dark { self.saved.dark_theme.clone() } else { self.saved.light_theme.clone() };
        if self.current != want { apply_now(&want, true)?; self.refresh(false); }
        Ok(())
    }

    fn name(&self, id: &str) -> String {
        self.themes.iter().find(|t| t.id == id).map(|t| t.name.clone()).unwrap_or_else(|| id.trim_end_matches(".desktop").to_string())
    }

    fn is_dark(&self) -> bool {
        if self.current == self.saved.dark_theme { return true; }
        if self.current == self.saved.light_theme { return false; }
        let c = self.current.to_lowercase();
        c.contains("dark") || c.contains("sombre") || c.contains("vapor")
    }

    fn theme_combo(&self, ui: &mut egui::Ui, id: &str, value: &mut String) {
        egui::ComboBox::from_id_salt(id).width(240.0).selected_text(self.name(value)).show_ui(ui, |ui| {
            for t in &self.themes { ui.selectable_value(value, t.id.clone(), &t.name); }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.refreshed.elapsed() > Duration::from_secs(5) { self.refresh(false); }
        ctx.request_repaint_after(Duration::from_secs(5));
        let dark = self.is_dark();
        let p = DARK;
        let now = Local::now();

        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.bg).inner_margin(Margin::same(20))).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                // En-tête
                ui.horizontal(|ui| {
                    if dark { moon_icon(ui, p.moon, p.bg) } else { sun_icon(ui, p.sun) }
                    ui.label(RichText::new("ClairObscur").size(24.0).strong());
                });
                let state = if dark { "sombre" } else { "clair" };
                ui.label(RichText::new(format!("Thème actuel : {} ({state})", self.name(&self.current))).color(p.muted));
                if self.saved.auto {
                    if let Some((to_dark, at)) = next_switch(&self.cycles, now.timestamp_millis()) {
                        let t = local(at);
                        let day = if t.date_naive() == now.date_naive() { "" } else { " demain" };
                        ui.label(RichText::new(format!("Prochain passage au {} :{day} à {}", if to_dark { "sombre" } else { "clair" }, t.format("%H:%M"))).color(p.muted));
                    }
                } else {
                    ui.label(RichText::new("Le basculement automatique est coupé.").color(p.muted));
                }
                ui.add_space(12.0);

                card(ui, &p, "Basculement", |ui| {
                    ui.checkbox(&mut self.edit.auto, "Passer seul du thème clair au thème sombre");
                    ui.add_enabled_ui(self.edit.auto, |ui| {
                        ui.add_space(4.0);
                        ui.radio_value(&mut self.edit.sun, true, "Au lever et au coucher du soleil");
                        if self.edit.sun {
                            if let Some((rise, set)) = today(&self.cycles, now).filter(|_| self.saved.sun) {
                                ui.indent("soleil", |ui| {
                                    ui.label(RichText::new(format!("Aujourd'hui : clair à {}, sombre à {}", local(rise).format("%H:%M"), local(set).format("%H:%M"))).size(13.0).color(p.muted));
                                });
                            }
                        }
                        ui.radio_value(&mut self.edit.sun, false, "Aux heures que je choisis");
                        if !self.edit.sun {
                            ui.add_space(4.0);
                            egui::Grid::new("heures").num_columns(3).spacing([10.0, 10.0]).show(ui, |ui| {
                                ui.add_space(14.0);
                                ui.horizontal(|ui| { sun_icon(ui, p.sun); ui.label("Clair à"); });
                                ui.horizontal(|ui| time_edit(ui, &mut self.edit.light_at));
                                ui.end_row();
                                ui.add_space(14.0);
                                ui.horizontal(|ui| { moon_icon(ui, p.moon, p.card); ui.label("Sombre à"); });
                                ui.horizontal(|ui| time_edit(ui, &mut self.edit.dark_at));
                                ui.end_row();
                            });
                            if self.edit.light_at == self.edit.dark_at {
                                ui.label(RichText::new("Les deux heures doivent être différentes.").size(13.0).color(Color32::from_rgb(0xE5, 0x4B, 0x4B)));
                            }
                        }
                    });
                });
                ui.add_space(10.0);

                card(ui, &p, "Thèmes", |ui| {
                    egui::Grid::new("themes").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                        ui.horizontal(|ui| { sun_icon(ui, p.sun); ui.label("Clair"); });
                        let mut v = self.edit.light_theme.clone();
                        self.theme_combo(ui, "clair", &mut v);
                        self.edit.light_theme = v;
                        ui.end_row();
                        ui.horizontal(|ui| { moon_icon(ui, p.moon, p.card); ui.label("Sombre"); });
                        let mut v = self.edit.dark_theme.clone();
                        self.theme_combo(ui, "sombre", &mut v);
                        self.edit.dark_theme = v;
                        ui.end_row();
                    });
                });
                ui.add_space(14.0);

                let dirty = self.edit != self.saved;
                let valid = self.edit.sun || self.edit.light_at != self.edit.dark_at;
                ui.horizontal(|ui| {
                    if button(ui, &p, dirty && valid, "Enregistrer", true, 150.0) {
                        let r = save_settings(&self.edit, &self.saved);
                        self.msg = Some(match &r { Ok(()) => (true, "Réglages enregistrés.".into()), Err(e) => (false, e.clone()) });
                        std::thread::sleep(Duration::from_millis(600)); // laisse knighttimed repartir
                        self.edit = read_settings().0;
                        self.refresh(true);
                        if r.is_err() { self.edit = self.saved.clone(); }
                        else if let Err(e) = self.enforce() { self.msg = Some((false, e)); }
                    }
                    if dirty && button(ui, &p, true, "Annuler", false, 110.0) { self.edit = self.saved.clone(); self.msg = None; }
                });
                if let Some((ok, m)) = &self.msg {
                    ui.label(RichText::new(m).size(13.0).color(if *ok { p.muted } else { Color32::from_rgb(0xE5, 0x4B, 0x4B) }));
                }
                ui.add_space(14.0);

                card(ui, &p, "Tout de suite", |ui| {
                    ui.horizontal(|ui| {
                        let (l, d) = (self.saved.light_theme.clone(), self.saved.dark_theme.clone());
                        let mut go = None;
                        if button(ui, &p, self.current != l, "☀  Clair", false, 130.0) { go = Some(l); }
                        if button(ui, &p, self.current != d, "☾  Sombre", false, 130.0) { go = Some(d); }
                        if let Some(t) = go {
                            if let Err(e) = apply_now(&t, self.saved.auto) { self.msg = Some((false, e)); }
                            self.refresh(false);
                        }
                    });
                    if self.saved.auto {
                        ui.label(RichText::new("Jusqu'au prochain passage automatique.").size(13.0).color(p.muted));
                    }
                });
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ini() {
        let m = parse_ini("[General]\nSource=Times\n\n[Times]\nSunriseStart=08:15\nName[fr]=x\n[Colors][Button]\nA=1\n");
        assert_eq!(get(&m, "General", "Source"), Some("Times"));
        assert_eq!(get(&m, "Times", "SunriseStart").and_then(parse_hhmm), Some((8, 15)));
        assert_eq!(get(&m, "Times", "Name"), None);
        assert_eq!(get(&m, "Colors][Button", "A"), Some("1"));
        assert_eq!(parse_hhmm("18:40:00"), Some((18, 40)));
    }

    #[test]
    fn schedule() {
        let j = r#"{"type":"a{sv}","data":[{"Cookie":{"type":"u","data":7},"Schedule":{"type":"(sv)","data":["dynamic",{"type":"a(xxxxx)","data":[[50,10,20,80,90],[150,110,120,180,190]]}]}}]}"#;
        let (cookie, c) = parse_schedule(j).unwrap();
        assert_eq!(cookie, 7);
        assert_eq!(c.len(), 2);
        assert_eq!(next_switch(&c, 15), Some((true, 85)));
        assert_eq!(next_switch(&c, 86), Some((false, 115)));
        assert_eq!(expected_dark(&c, 5), None);
        assert_eq!(expected_dark(&c, 20), Some(false));
        assert_eq!(expected_dark(&c, 100), Some(true));
    }
}

fn main() -> eframe::Result {
    let icon = image::load_from_memory(include_bytes!("../resources/icon.png")).ok().map(|i| {
        let i = i.to_rgba8();
        let (w, h) = i.dimensions();
        egui::IconData { rgba: i.into_raw(), width: w, height: h }
    });
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("ClairObscur").with_app_id("clairobscur").with_inner_size([520.0, 640.0]).with_min_inner_size([460.0, 520.0]);
    if let Some(i) = icon { viewport = viewport.with_icon(i); }
    let options = eframe::NativeOptions { viewport, renderer: eframe::Renderer::Glow, vsync: true, ..Default::default() };
    eframe::run_native("ClairObscur", options, Box::new(|cc| { style(&cc.egui_ctx, &DARK); Ok(Box::new(App::new())) }))
}
