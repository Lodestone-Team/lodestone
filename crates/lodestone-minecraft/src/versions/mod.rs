//! Minecraft and mod loader versions: the shapes of the version APIs (Mojang, Fabric,
//! Forge) and the choice of a version from their answers. Fetching them is up to the
//! caller.

use std::cmp::Ordering;

pub mod fabric;
pub mod forge;
pub mod mojang;

/// Compares version strings such as "0.14.21" or "1.0.0-beta.2" part by part, numerically
/// where both parts are numbers. If one is a prefix of the other, the longer one is newer.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let split = |s: &str| -> Vec<String> {
        s.split(['.', '-', '+', '_'])
            .map(|p| p.to_string())
            .collect()
    };
    let (a_parts, b_parts) = (split(a), split(b));
    for (a_part, b_part) in a_parts.iter().zip(b_parts.iter()) {
        let ordering = match (a_part.parse::<u64>(), b_part.parse::<u64>()) {
            (Ok(a_num), Ok(b_num)) => a_num.cmp(&b_num),
            _ => a_part.cmp(b_part),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    a_parts.len().cmp(&b_parts.len())
}

/// Recorded API responses, trimmed to what the tests need.
#[cfg(test)]
pub(crate) fn fixture<T: serde::de::DeserializeOwned>(json: &str) -> T {
    serde_json::from_str(json).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions() {
        assert_eq!(compare_versions("0.14.21", "0.14.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.2", "1.2.1"), Ordering::Less);
        assert_eq!(compare_versions("1.2.1", "1.2.1"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0", "0.11.2"), Ordering::Greater);
        assert_eq!(compare_versions("x.1", "1.1"), Ordering::Greater);
    }
}
