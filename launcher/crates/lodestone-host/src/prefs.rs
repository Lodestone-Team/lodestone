//! The Host's own settings, kept on this computer (the dashboard keeps them in the
//! browser's `localStorage`): the theme, the language, the zoom and the coloured
//! console, and the window's layout. They live in `settings.json` in the Host's folder
//! ([`crate::core_process::host_dir`]); the core's own settings stay on the core.
//!
//! [`Prefs`] is a GPUI global. Read it with `Prefs::get(cx)`; change it with
//! [`Prefs::update`], which saves the file and redraws the windows.

use std::path::{Path, PathBuf};

use gpui::{App, Global, WindowAppearance};
use launcher_core::settings::Language;
use serde::{Deserialize, Serialize};

/// The theme: as the computer's, or always light or dark (`utils/theme.ts`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    /// Last, as serde wants of the variant that reads unknown values.
    #[default]
    #[serde(other)]
    System,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];

    /// Whether the window is light with the computer's `appearance`.
    pub fn is_light(self, appearance: WindowAppearance) -> bool {
        match self {
            Theme::Light => true,
            Theme::Dark => false,
            Theme::System => matches!(
                appearance,
                WindowAppearance::Light | WindowAppearance::VibrantLight
            ),
        }
    }
}

/// What is saved. Every field has a default, so an older or damaged file still loads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrefsData {
    pub theme: Theme,
    pub language: Language,
    /// 1.0 is 100%.
    pub zoom: f32,
    /// Console colours (the dashboard's `coloredConsole`).
    pub colored_console: bool,
    /// The Overview's Details section is open (`instanceDetailsOpen`).
    pub instance_details_open: bool,
    /// Settings › Updates: download new builds by themselves.
    pub auto_update: bool,
    /// Settings › Updates: beta builds too.
    pub beta_builds: bool,
    /// The last beta core version thanked for (`latestVersion`).
    pub latest_version: Option<String>,
    /// Other cores connected to before (the dashboard's `cores`), with the certificate
    /// trusted for each. Their sign-ins aren't saved.
    pub cores: Vec<SavedCore>,
}

/// Another core: its address and the HTTPS certificate the user trusted for it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedCore {
    /// As `launcher_core::cores::client::display_address` writes it.
    pub address: String,
    /// SHA-256, as the core shows it.
    pub fingerprint: Option<String>,
}

impl Default for PrefsData {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            language: Language::System,
            zoom: 1.0,
            colored_console: true,
            instance_details_open: false,
            auto_update: true,
            beta_builds: false,
            latest_version: None,
            cores: Vec::new(),
        }
    }
}

pub struct Prefs {
    pub data: PrefsData,
    path: Option<PathBuf>,
}

impl Global for Prefs {}

impl Prefs {
    /// Loads the settings from `<host folder>/settings.json` and applies them: the
    /// language and the zoom. The theme follows once the window knows its appearance
    /// ([`Prefs::apply_theme`]). Call once at startup.
    pub fn init(cx: &mut App) {
        let path = crate::core_process::host_dir().join("settings.json");
        let data = load(&path);
        lodestone_ui::i18n::apply(data.language);
        lodestone_ui::theme::zoom::set(data.zoom);
        cx.set_global(Prefs {
            data,
            path: Some(path),
        });
    }

    pub fn get(cx: &App) -> &PrefsData {
        &cx.global::<Prefs>().data
    }

    /// Changes the settings, saves them, applies the language, zoom and theme, and
    /// redraws the windows.
    pub fn update(cx: &mut App, change: impl FnOnce(&mut PrefsData)) {
        let language_before = cx.global::<Prefs>().data.language;
        let prefs = cx.global_mut::<Prefs>();
        change(&mut prefs.data);
        prefs.data.zoom = lodestone_ui::theme::zoom::clamp(prefs.data.zoom);
        if let Some(path) = &prefs.path
            && let Err(err) = save(path, &prefs.data)
        {
            eprintln!("could not save {}: {err}", path.display());
        }
        let data = prefs.data.clone();
        if data.language != language_before {
            lodestone_ui::i18n::apply(data.language);
            crate::set_menus(cx);
        }
        lodestone_ui::theme::zoom::set(data.zoom);
        Self::apply_theme(cx.window_appearance(), cx);
        cx.refresh_windows();
    }

    /// Turns the light theme on or off for the theme setting and the computer's
    /// `appearance`. Returns whether it changed.
    pub fn apply_theme(appearance: WindowAppearance, cx: &App) -> bool {
        let light = cx
            .try_global::<Prefs>()
            .map_or(Theme::System, |p| p.data.theme)
            .is_light(appearance);
        let changed = lodestone_ui::theme::palette::is_light() != light;
        lodestone_ui::theme::palette::set_light(light);
        changed
    }
}

fn load(path: &Path) -> PrefsData {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save(path: &Path, data: &PrefsData) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(data).map_err(std::io::Error::other)?;
    // write beside it, then move it in place, so a crash never leaves half a file
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, json)?;
    launcher_core::fsx::rename(&temp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_tolerate_old_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(load(&path), PrefsData::default(), "no file yet");
        let data = PrefsData {
            theme: Theme::Light,
            language: Language::French,
            zoom: 1.25,
            colored_console: false,
            instance_details_open: true,
            auto_update: false,
            beta_builds: true,
            latest_version: Some("0.5.2-beta.1".into()),
            cores: vec![SavedCore {
                address: "https://192.168.1.5:16662".into(),
                fingerprint: Some("AB:CD".into()),
            }],
        };
        save(&path, &data).unwrap();
        assert_eq!(load(&path), data);
        std::fs::write(&path, r#"{"theme":"sepia","zoom":0.9,"something":1}"#).unwrap();
        let old = load(&path);
        assert_eq!(old.theme, Theme::System);
        assert_eq!(old.zoom, 0.9);
        assert!(old.colored_console);
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(load(&path), PrefsData::default());
    }

    #[test]
    fn the_theme_follows_the_computer_when_asked() {
        assert!(Theme::System.is_light(WindowAppearance::Light));
        assert!(!Theme::System.is_light(WindowAppearance::VibrantDark));
        assert!(Theme::Light.is_light(WindowAppearance::Dark));
        assert!(!Theme::Dark.is_light(WindowAppearance::Light));
    }
}
