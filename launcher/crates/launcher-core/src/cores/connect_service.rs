//! Which Lodestone Connect introduction service ("relay") the apps use: the one source
//! of truth for Lodestone Launcher and Lodestone Host.
//!
//! In order, the first that is set and valid wins:
//!
//! 1. the `LODESTONE_CONNECT_SERVICE` environment variable ([`ENV`]);
//! 2. the app's developer setting ([`set_setting`]: the launcher's Settings ›
//!    Developer, the Host's Settings › Updates › Developer);
//! 3. connect.lodestone.cc ([`DEFAULT_SERVICE`]).
//!
//! A value must be `https://name[:port]`, or `http://` to this computer
//! (`http://127.0.0.1:8787` for `wrangler dev`), with nothing after the address
//! ([`Service::from_setting`]). An invalid one is warned about and skipped.
//!
//! Invite links name their own service, and joining one always goes through the
//! service in the link: these settings choose where new things go (the friend list,
//! the Host's core's invite links).

use std::sync::RwLock;

pub use lodestone_connect::{DEFAULT_SERVICE, Service, SettingError};

/// The environment variable that overrides the setting. The core reads it too.
pub const ENV: &str = "LODESTONE_CONNECT_SERVICE";

/// The app's developer setting, set at startup and when it changes.
static SETTING: RwLock<Option<Service>> = RwLock::new(None);

/// Where the service in use comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// [`ENV`].
    Env,
    /// The app's developer setting.
    Setting,
    /// Nothing is set: connect.lodestone.cc.
    Default,
}

/// The service in use, and where it comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Active {
    pub service: Service,
    pub source: Source,
}

impl Active {
    /// Not connect.lodestone.cc: the apps say so, so testers don't forget.
    pub fn is_custom(&self) -> bool {
        self.service != Service::default()
    }

    /// The service's address as a URL: `https://connect-dev.lodestone.cc`.
    pub fn url(&self) -> String {
        self.service.base_url()
    }
}

/// Checks a service URL as typed into a setting. Empty text means the default
/// (`Ok(None)`).
pub fn parse(text: &str) -> Result<Option<Service>, SettingError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    Service::from_setting(text).map(|service| (service != Service::default()).then_some(service))
}

/// The service for an environment value and a saved setting (both as text, either
/// missing), in the order the module describes. Invalid values are warned about on
/// stderr and skipped.
pub fn resolve(env: Option<&str>, setting: Option<&str>) -> Active {
    let checked = |text: Option<&str>, what: &str| {
        let text = text.map(str::trim).filter(|text| !text.is_empty())?;
        match Service::from_setting(text) {
            Ok(service) => Some(service),
            Err(err) => {
                eprintln!("{what} ({text}) isn't a Lodestone Connect service: {err}; skipped");
                None
            }
        }
    };
    if let Some(service) = checked(env, ENV) {
        return Active {
            service,
            source: Source::Env,
        };
    }
    if let Some(service) = checked(setting, "The Connect service setting") {
        return Active {
            service,
            source: Source::Setting,
        };
    }
    Active {
        service: Service::default(),
        source: Source::Default,
    }
}

/// The service in use now: [`ENV`], else the setting, else connect.lodestone.cc.
pub fn active() -> Active {
    let env = std::env::var(ENV).ok();
    let setting = setting().map(|service| service.base_url());
    resolve(env.as_deref(), setting.as_deref())
}

/// The service to use now. Read it when it is needed (each lookup, each new link),
/// not once, so a changed setting applies without a restart.
pub fn service() -> Service {
    active().service
}

/// Whether [`ENV`] holds a valid service, which then wins over the setting.
pub fn env_overrides() -> bool {
    active().source == Source::Env
}

/// The app's developer setting, if set.
pub fn setting() -> Option<Service> {
    SETTING.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Sets the app's developer setting for this process: `None` (or the default
/// service) for connect.lodestone.cc. The app saves it in its own settings file and
/// calls this at startup and when it changes.
pub fn set_setting(service: Option<Service>) {
    let service = service.filter(|service| *service != Service::default());
    *SETTING.write().unwrap_or_else(|e| e.into_inner()) = service;
}

/// Reads a saved setting's text (from a settings file) and applies it. Invalid text
/// is warned about and ignored.
pub fn set_setting_text(text: Option<&str>) {
    let service = text.and_then(|text| match parse(text) {
        Ok(service) => service,
        Err(err) => {
            eprintln!("The saved Connect service ({text}) isn't valid: {err}; using the default");
            None
        }
    });
    set_setting(service);
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEV: &str = "https://connect-dev.lodestone.cc";
    const LOCAL: &str = "http://127.0.0.1:8787";

    #[test]
    fn the_environment_wins_over_the_setting_which_wins_over_the_default() {
        let both = resolve(Some(LOCAL), Some(DEV));
        assert_eq!((both.url(), both.source), (LOCAL.to_string(), Source::Env));
        let setting = resolve(None, Some(DEV));
        assert_eq!(
            (setting.url(), setting.source),
            (DEV.to_string(), Source::Setting)
        );
        assert!(setting.is_custom());
        let none = resolve(None, None);
        assert_eq!(none.service, Service::default());
        assert_eq!(none.source, Source::Default);
        assert!(!none.is_custom());
        // blank counts as unset
        assert_eq!(resolve(Some("  "), Some(DEV)).source, Source::Setting);
    }

    #[test]
    fn invalid_values_are_skipped() {
        // a bad environment value falls back to the setting, a bad setting to the default
        let bad_env = resolve(Some("http://connect-dev.lodestone.cc"), Some(DEV));
        assert_eq!(bad_env.source, Source::Setting);
        let bad_both = resolve(Some("nope"), Some("https://a.example/path"));
        assert_eq!(bad_both.source, Source::Default);
        assert_eq!(bad_both.service, Service::default());
    }

    #[test]
    fn typed_text_is_checked() {
        assert_eq!(parse(""), Ok(None));
        assert_eq!(parse("  "), Ok(None));
        assert_eq!(
            parse(DEFAULT_SERVICE),
            Ok(None),
            "the default is no setting"
        );
        assert_eq!(
            parse(" https://connect-dev.lodestone.cc/ ").map(|s| s.map(|s| s.base_url())),
            Ok(Some(DEV.to_string()))
        );
        assert_eq!(
            parse("http://localhost:8787").map(|s| s.map(|s| s.base_url())),
            Ok(Some("http://localhost:8787".to_string()))
        );
        assert_eq!(
            parse("http://connect-dev.lodestone.cc"),
            Err(SettingError::Insecure)
        );
        assert_eq!(parse("connect-dev.lodestone.cc"), Err(SettingError::Scheme));
        assert_eq!(parse("https://a.example/x"), Err(SettingError::Extra));
        assert_eq!(parse("https://a b"), Err(SettingError::Address));
    }

    #[test]
    fn the_setting_is_kept_for_the_process() {
        // the only test that touches the global
        set_setting_text(Some(DEV));
        assert_eq!(setting().map(|s| s.base_url()), Some(DEV.to_string()));
        set_setting_text(Some("http://example.com"));
        assert_eq!(setting(), None, "invalid text clears it");
        set_setting(Service::from_setting(DEFAULT_SERVICE).ok());
        assert_eq!(setting(), None, "the default is no setting");
        set_setting(None);
    }
}
