//! Mojang's `rules`: whether a library or an argument applies on a platform with a
//! set of launcher features.
//!
//! A list of rules starts as "disallowed". Each rule whose conditions all match sets
//! the result to its action, so the last matching rule wins. An empty list allows.
//! A rule's conditions are `os` (`name`, `arch`, a `version` regex, and the newer
//! `versionRange`) and `features`.

use std::{cmp::Ordering, collections::BTreeMap};

use lodestone_minecraft::versions::compare_versions;
use serde::Deserialize;

use super::platform::Platform;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Allow,
    Disallow,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    /// A regular expression the OS version must match.
    pub version: Option<String>,
    #[serde(rename = "versionRange")]
    pub version_range: Option<VersionRange>,
}

/// An OS version range: `min` is inclusive and `max` exclusive, so Mojang's paired
/// rules (`max: X` on one, `min: X` on the other) never both match.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct VersionRange {
    pub min: Option<String>,
    pub max: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Rule {
    pub action: Action,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<BTreeMap<String, bool>>,
}

/// The launcher features that `features` rules ask about.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Features {
    pub is_demo_user: bool,
    pub has_custom_resolution: bool,
    pub has_quick_plays_support: bool,
    pub is_quick_play_singleplayer: bool,
    pub is_quick_play_multiplayer: bool,
    pub is_quick_play_realms: bool,
}

impl Features {
    /// The value of a feature by its name in the version file. Unknown features are
    /// off, as in the official launcher.
    pub fn get(&self, name: &str) -> bool {
        match name {
            "is_demo_user" => self.is_demo_user,
            "has_custom_resolution" => self.has_custom_resolution,
            "has_quick_plays_support" => self.has_quick_plays_support,
            "is_quick_play_singleplayer" => self.is_quick_play_singleplayer,
            "is_quick_play_multiplayer" => self.is_quick_play_multiplayer,
            "is_quick_play_realms" => self.is_quick_play_realms,
            _ => false,
        }
    }
}

impl OsRule {
    pub fn matches(&self, platform: &Platform) -> bool {
        if let Some(name) = &self.name
            && name != platform.os.rule_name()
        {
            return false;
        }
        if let Some(arch) = &self.arch
            && !arch_matches(arch, platform)
        {
            return false;
        }
        if let Some(pattern) = &self.version {
            // An invalid pattern matches nothing, so the rule is skipped.
            let matched = regex::RegexBuilder::new(pattern)
                .unicode(false)
                .build()
                .is_ok_and(|regex| regex.is_match(&platform.os_version));
            if !matched {
                return false;
            }
        }
        if let Some(range) = &self.version_range {
            let version = platform.os_version.as_str();
            if let Some(min) = &range.min
                && compare_versions(version, min) == Ordering::Less
            {
                return false;
            }
            if let Some(max) = &range.max
                && compare_versions(version, max) != Ordering::Less
            {
                return false;
            }
        }
        true
    }
}

/// Mojang's `arch` values: `x86` is a 32-bit JVM. Newer files may also name 64-bit
/// architectures, which are compared by their usual spellings.
fn arch_matches(arch: &str, platform: &Platform) -> bool {
    use super::platform::Arch;
    matches!(
        (arch, platform.arch),
        ("x86", Arch::X86)
            | ("x86_64" | "amd64" | "x64", Arch::X86_64)
            | ("arm64" | "aarch64", Arch::Arm64)
    )
}

impl Rule {
    /// Whether all of the rule's conditions hold.
    pub fn matches(&self, platform: &Platform, features: &Features) -> bool {
        if let Some(os) = &self.os
            && !os.matches(platform)
        {
            return false;
        }
        if let Some(wanted) = &self.features
            && !wanted
                .iter()
                .all(|(name, value)| features.get(name) == *value)
        {
            return false;
        }
        true
    }
}

/// Whether `rules` allow something on `platform` with `features`.
pub fn allows(rules: &[Rule], platform: &Platform, features: &Features) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule.matches(platform, features) {
            allowed = rule.action == Action::Allow;
        }
    }
    allowed
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::game::platform::{Arch, Os};

    fn rules(value: serde_json::Value) -> Vec<Rule> {
        serde_json::from_value(value).unwrap()
    }

    fn mac(arch: Arch) -> Platform {
        Platform::new(Os::MacOs, arch, "14.6.1")
    }

    #[test]
    fn no_rules_allow() {
        assert!(allows(&[], &mac(Arch::Arm64), &Features::default()));
    }

    #[test]
    fn os_names() {
        let only_mac = rules(json!([{"action": "allow", "os": {"name": "osx"}}]));
        let not_mac = rules(json!([
            {"action": "allow"},
            {"action": "disallow", "os": {"name": "osx"}}
        ]));
        let f = Features::default();
        let linux = Platform::new(Os::Linux, Arch::X86_64, "6.1");
        let windows = Platform::new(Os::Windows, Arch::X86_64, "10.0");
        assert!(allows(&only_mac, &mac(Arch::Arm64), &f));
        assert!(allows(&only_mac, &mac(Arch::X86_64), &f));
        assert!(!allows(&only_mac, &linux, &f));
        assert!(!allows(&not_mac, &mac(Arch::Arm64), &f));
        assert!(allows(&not_mac, &linux, &f));
        assert!(allows(&not_mac, &windows, &f));
    }

    #[test]
    fn arch() {
        let x86 = rules(json!([{"action": "allow", "os": {"arch": "x86"}}]));
        let f = Features::default();
        assert!(!allows(&x86, &mac(Arch::X86_64), &f));
        assert!(!allows(&x86, &mac(Arch::Arm64), &f));
        assert!(allows(
            &x86,
            &Platform::new(Os::Windows, Arch::X86, "10.0"),
            &f
        ));
        let arm = rules(json!([{"action": "allow", "os": {"name": "osx", "arch": "arm64"}}]));
        assert!(allows(&arm, &mac(Arch::Arm64), &f));
        assert!(!allows(&arm, &mac(Arch::X86_64), &f));
    }

    #[test]
    fn version_regex() {
        // From old version files: a library only for Mac OS X 10.5.
        let leopard = rules(json!([
            {"action": "allow"},
            {"action": "disallow", "os": {"name": "osx", "version": "^10\\.5\\.\\d$"}}
        ]));
        let f = Features::default();
        assert!(!allows(
            &leopard,
            &Platform::new(Os::MacOs, Arch::X86_64, "10.5.8"),
            &f
        ));
        assert!(allows(
            &leopard,
            &Platform::new(Os::MacOs, Arch::X86_64, "10.15.7"),
            &f
        ));
        assert!(allows(&leopard, &mac(Arch::Arm64), &f));
        // An invalid pattern never matches.
        let invalid = rules(json!([{"action": "allow", "os": {"version": "(unclosed"}}]));
        assert!(!allows(&invalid, &mac(Arch::Arm64), &f));
    }

    #[test]
    fn version_ranges() {
        // From 26.3's `default-user-jvm` arguments.
        let new_windows = rules(
            json!([{"action": "allow", "os": {"name": "windows", "versionRange": {"min": "10.0.17134"}}}]),
        );
        let old_windows = rules(
            json!([{"action": "allow", "os": {"name": "windows", "versionRange": {"max": "10.0.17134"}}}]),
        );
        let f = Features::default();
        let windows = |version| Platform::new(Os::Windows, Arch::X86_64, version);
        assert!(allows(&new_windows, &windows("10.0.22631"), &f));
        assert!(allows(&new_windows, &windows("10.0.17134"), &f));
        assert!(!allows(&old_windows, &windows("10.0.17134"), &f));
        assert!(allows(&old_windows, &windows("10.0.10240"), &f));
        assert!(!allows(&new_windows, &mac(Arch::Arm64), &f));
    }

    #[test]
    fn features() {
        let resolution = rules(json!([
            {"action": "allow", "features": {"has_custom_resolution": true}}
        ]));
        let demo = rules(json!([{"action": "allow", "features": {"is_demo_user": true}}]));
        let unknown = rules(json!([{"action": "allow", "features": {"is_new_feature": true}}]));
        let p = mac(Arch::Arm64);
        let off = Features::default();
        let on = Features {
            has_custom_resolution: true,
            ..Features::default()
        };
        assert!(!allows(&resolution, &p, &off));
        assert!(allows(&resolution, &p, &on));
        assert!(!allows(&demo, &p, &on));
        assert!(!allows(&unknown, &p, &on));
        // A rule asking for a feature to be off matches when it is.
        let not_demo = rules(json!([{"action": "allow", "features": {"is_demo_user": false}}]));
        assert!(allows(&not_demo, &p, &off));
    }

    #[test]
    fn last_matching_rule_wins() {
        let r = rules(json!([
            {"action": "disallow"},
            {"action": "allow", "os": {"name": "osx"}},
            {"action": "disallow", "os": {"name": "osx", "arch": "x86_64"}}
        ]));
        let f = Features::default();
        assert!(allows(&r, &mac(Arch::Arm64), &f));
        assert!(!allows(&r, &mac(Arch::X86_64), &f));
    }
}
