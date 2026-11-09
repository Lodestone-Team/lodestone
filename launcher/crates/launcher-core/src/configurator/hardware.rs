//! What the computer has, for [`super`]: its memory, how many threads its processor
//! runs, and its graphics card when the system says.
//!
//! Detection is best effort and never fails: what can't be read is `None`, and the
//! recommendations fall back to cautious values.

use std::sync::OnceLock;

use serde::Serialize;

use crate::game::platform::{self, Arch, Os};

/// The computer, as the recommendations see it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Hardware {
    /// Installed memory, bytes.
    pub total_memory: Option<u64>,
    /// Threads the processor runs at once.
    pub threads: u32,
    /// The graphics card the game most likely draws with.
    pub gpu: Option<Gpu>,
}

/// A graphics card.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Gpu {
    pub name: String,
    pub kind: GpuKind,
    /// Its own memory, bytes, when the system says.
    pub memory: Option<u64>,
}

/// What kind of graphics card it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuKind {
    /// A card of its own (GeForce, Radeon RX, Arc A-series).
    Discrete,
    /// Graphics built into the processor, sharing its memory (Intel UHD and Iris,
    /// Radeon Graphics in a Ryzen).
    Integrated,
    /// An Apple silicon Mac: built in, but much faster than other integrated graphics.
    AppleSilicon,
}

const GIB: u64 = 1 << 30;

impl Hardware {
    /// This computer's, read once and kept.
    pub fn current() -> &'static Hardware {
        static HARDWARE: OnceLock<Hardware> = OnceLock::new();
        HARDWARE.get_or_init(Hardware::detect)
    }

    fn detect() -> Self {
        Self {
            total_memory: platform::total_memory(),
            threads: std::thread::available_parallelism()
                .map(|n| n.get() as u32)
                .unwrap_or(4),
            gpu: detect_gpu(),
        }
    }

    /// Installed memory in MiB, when known.
    pub fn total_mb(&self) -> Option<u32> {
        self.total_memory
            .map(|bytes| (bytes / (1024 * 1024)).min(u64::from(u32::MAX)) as u32)
    }

    fn total_gib(&self) -> Option<u64> {
        // Round, so 15.8 GiB usable reads as 16.
        self.total_memory.map(|bytes| (bytes + GIB / 2) / GIB)
    }
}

/// How well the computer draws the game, from its graphics card, processor and memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Low,
    Medium,
    High,
    Ultra,
}

impl Tier {
    /// The tier of `hardware`.
    pub fn of(hardware: &Hardware) -> Tier {
        let ram = hardware.total_gib();
        let threads = hardware.threads;
        let gpu = match &hardware.gpu {
            Some(Gpu {
                kind: GpuKind::Discrete,
                memory,
                ..
            }) => match memory.map(|m| (m + GIB / 2) / GIB) {
                Some(8..) => Tier::Ultra,
                Some(4..) => Tier::High,
                // A card that doesn't say its memory is still a card of its own.
                None => Tier::High,
                Some(_) => Tier::Medium,
            },
            Some(Gpu {
                kind: GpuKind::AppleSilicon,
                ..
            }) => {
                if ram.is_some_and(|r| r >= 16) {
                    Tier::High
                } else {
                    Tier::Medium
                }
            }
            Some(Gpu {
                kind: GpuKind::Integrated,
                ..
            }) => Tier::Low,
            None => {
                if ram.is_some_and(|r| r >= 16) && threads >= 8 {
                    Tier::Medium
                } else {
                    Tier::Low
                }
            }
        };
        // The processor and memory hold back a fast card.
        let cpu = match threads {
            0..=3 => Tier::Low,
            4..=5 => Tier::Medium,
            6..=7 => Tier::High,
            _ => Tier::Ultra,
        };
        let memory = match ram {
            Some(0..=5) => Tier::Low,
            Some(6..=15) => Tier::High,
            Some(_) => Tier::Ultra,
            None => Tier::Medium,
        };
        gpu.min(cpu).min(memory)
    }
}

/// The kind of a card from its name and memory.
pub fn classify(name: &str, memory: Option<u64>) -> Option<GpuKind> {
    let lower = name.to_ascii_lowercase();
    let big = memory.is_some_and(|m| m >= 2 * GIB);
    // Virtual and fallback adapters draw nothing themselves.
    let virtual_adapter = [
        "basic display",
        "basic render",
        "remote display",
        "virtual",
        "parsec",
        "citrix",
        "vmware",
        "virtualbox",
        "hyper-v",
        "spacedesk",
        "iddsample",
        "indirect display",
    ];
    if virtual_adapter.iter().any(|v| lower.contains(v)) {
        return None;
    }
    if lower.contains("nvidia") || lower.contains("geforce") || lower.contains("quadro") {
        return Some(if lower.contains("tegra") {
            GpuKind::Integrated
        } else {
            GpuKind::Discrete
        });
    }
    if lower.contains("intel") {
        // "Intel(R) Arc(TM) A770 Graphics" is a card; "Intel(R) Arc(TM) Graphics" is
        // built into a Core Ultra.
        let arc_card = lower.contains("arc")
            && lower
                .split(|c: char| !c.is_ascii_alphanumeric())
                .any(|word| {
                    word.len() == 4
                        && (word.starts_with('a') || word.starts_with('b'))
                        && word[1..].chars().all(|c| c.is_ascii_digit())
                });
        return Some(if arc_card {
            GpuKind::Discrete
        } else {
            GpuKind::Integrated
        });
    }
    if lower.contains("amd") || lower.contains("radeon") || lower.contains("ati ") {
        let card = [
            " rx ",
            " rx",
            "radeon pro",
            " r9 ",
            " r7 ",
            "radeon vii",
            "fury",
            "firepro",
        ]
        .iter()
        .any(|word| lower.contains(word))
            && !lower.contains("rx vega 1")
            && !lower.contains("vega 8")
            && !lower.contains("vega 3");
        return Some(if card || big {
            GpuKind::Discrete
        } else {
            GpuKind::Integrated
        });
    }
    if lower.contains("apple") {
        return Some(GpuKind::AppleSilicon);
    }
    big.then_some(GpuKind::Discrete)
}

/// The card the game most likely draws with: a card of its own over built-in graphics,
/// then the one with the most memory.
pub fn best(gpus: impl IntoIterator<Item = Gpu>) -> Option<Gpu> {
    let rank = |gpu: &Gpu| {
        let kind = match gpu.kind {
            GpuKind::Discrete => 2,
            GpuKind::AppleSilicon => 1,
            GpuKind::Integrated => 0,
        };
        (kind, gpu.memory.unwrap_or(0))
    };
    gpus.into_iter().max_by_key(rank)
}

fn detect_gpu() -> Option<Gpu> {
    if Os::current() == Os::MacOs {
        // Every Apple silicon Mac draws with its own graphics. Intel Macs have
        // either Intel graphics or a Radeon, which only IOKit says.
        return (Arch::current() == Arch::Arm64).then(|| Gpu {
            name: "Apple silicon".into(),
            kind: GpuKind::AppleSilicon,
            memory: None,
        });
    }
    best(system_gpus())
}

/// The display adapters Windows has drivers for, from the registry's display class
/// (what Device Manager lists under Display adapters).
#[cfg(windows)]
fn system_gpus() -> Vec<Gpu> {
    const CLASS: &str =
        r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
    let mut gpus = Vec::new();
    for index in 0..16 {
        let key = format!(r"{CLASS}\{index:04}");
        let Some(name) = registry::string(&key, "DriverDesc") else {
            continue;
        };
        let memory = registry::number(&key, "HardwareInformation.qwMemorySize")
            .or_else(|| registry::number(&key, "HardwareInformation.MemorySize"))
            .filter(|m| *m > 0);
        if let Some(kind) = classify(&name, memory) {
            gpus.push(Gpu { name, kind, memory });
        }
    }
    gpus
}

/// The cards in `/sys/class/drm`, by PCI vendor: NVIDIA's are cards of their own,
/// Intel's built in, and AMD's a card when they have memory of their own.
#[cfg(target_os = "linux")]
fn system_gpus() -> Vec<Gpu> {
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };
    let read = |path: std::path::PathBuf| {
        std::fs::read_to_string(path)
            .ok()
            .map(|s| s.trim().to_string())
    };
    let mut gpus = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // card0, card1: not their outputs (card0-HDMI-A-1).
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        let device = entry.path().join("device");
        let Some(vendor) = read(device.join("vendor")) else {
            continue;
        };
        let memory = read(device.join("mem_info_vram_total")).and_then(|v| v.parse().ok());
        let (name, kind) = match vendor.as_str() {
            "0x10de" => ("NVIDIA", GpuKind::Discrete),
            "0x8086" => ("Intel", GpuKind::Integrated),
            "0x1002" if memory.is_some_and(|m: u64| m >= 2 * GIB) => ("AMD", GpuKind::Discrete),
            "0x1002" => ("AMD", GpuKind::Integrated),
            _ => continue,
        };
        gpus.push(Gpu {
            name: name.into(),
            kind,
            memory,
        });
    }
    gpus
}

#[cfg(not(any(windows, target_os = "linux")))]
fn system_gpus() -> Vec<Gpu> {
    Vec::new()
}

#[cfg(windows)]
mod registry {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{HKEY_LOCAL_MACHINE, REG_SZ, RRF_RT_ANY, RegGetValueW},
    };

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// A value of `HKEY_LOCAL_MACHINE\<key>`, its type and bytes.
    fn value(key: &str, name: &str) -> Option<(u32, Vec<u8>)> {
        let key = wide(key);
        let name = wide(name);
        let mut kind = 0u32;
        let mut bytes = 0u32;
        // SAFETY: a size query; nothing is written but `kind` and `bytes`.
        let sized = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_ANY,
                &mut kind,
                std::ptr::null_mut(),
                &mut bytes,
            )
        };
        if sized != ERROR_SUCCESS || bytes == 0 || bytes > 4096 {
            return None;
        }
        let mut buffer = vec![0u8; bytes as usize];
        // SAFETY: `buffer` holds `bytes` bytes, which is all the call writes.
        let read = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_ANY,
                &mut kind,
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if read != ERROR_SUCCESS {
            return None;
        }
        buffer.truncate(bytes as usize);
        Some((kind, buffer))
    }

    pub fn string(key: &str, name: &str) -> Option<String> {
        let (kind, bytes) = value(key, name)?;
        if kind != REG_SZ {
            return None;
        }
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        let text = String::from_utf16_lossy(&units);
        let text = text.trim_end_matches('\0').trim();
        (!text.is_empty()).then(|| text.to_string())
    }

    /// A DWORD, QWORD or little-endian binary value (drivers write the card's memory as
    /// any of them).
    pub fn number(key: &str, name: &str) -> Option<u64> {
        let (_, bytes) = value(key, name)?;
        if bytes.is_empty() || bytes.len() > 8 {
            return None;
        }
        let mut le = [0u8; 8];
        le[..bytes.len()].copy_from_slice(&bytes);
        Some(u64::from_le_bytes(le))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(kind: GpuKind, gib: Option<u64>) -> Option<Gpu> {
        Some(Gpu {
            name: String::new(),
            kind,
            memory: gib.map(|g| g * GIB),
        })
    }

    fn hw(ram_gib: u64, threads: u32, gpu: Option<Gpu>) -> Hardware {
        Hardware {
            total_memory: Some(ram_gib * GIB),
            threads,
            gpu,
        }
    }

    /// Reads this computer, which never fails (`--nocapture` shows what it found).
    #[test]
    fn detects_this_computer() {
        let hardware = Hardware::detect();
        println!("{hardware:?} -> {:?}", Tier::of(&hardware));
        assert!(hardware.threads > 0);
        if cfg!(any(windows, target_os = "macos", target_os = "linux")) {
            assert!(hardware.total_memory.is_some());
        }
    }

    #[test]
    fn classifies_cards_by_name() {
        let k = |name: &str, gib: u64| classify(name, Some(gib * GIB));
        assert_eq!(k("NVIDIA GeForce RTX 4070", 12), Some(GpuKind::Discrete));
        assert_eq!(k("NVIDIA GeForce GTX 1050 Ti", 4), Some(GpuKind::Discrete));
        assert_eq!(k("Intel(R) UHD Graphics 620", 1), Some(GpuKind::Integrated));
        assert_eq!(
            k("Intel(R) Iris(R) Xe Graphics", 1),
            Some(GpuKind::Integrated)
        );
        assert_eq!(k("Intel(R) Arc(TM) Graphics", 1), Some(GpuKind::Integrated));
        assert_eq!(
            k("Intel(R) Arc(TM) A770 Graphics", 16),
            Some(GpuKind::Discrete)
        );
        assert_eq!(k("AMD Radeon(TM) Graphics", 0), Some(GpuKind::Integrated));
        assert_eq!(k("AMD Radeon RX 7800 XT", 16), Some(GpuKind::Discrete));
        assert_eq!(
            k("AMD Radeon RX Vega 11 Graphics", 0),
            Some(GpuKind::Integrated)
        );
        assert_eq!(k("Microsoft Basic Display Adapter", 0), None);
        assert_eq!(k("Parsec Virtual Display Adapter", 0), None);
        assert_eq!(classify("Some Card", None), None);
    }

    #[test]
    fn prefers_a_card_of_its_own() {
        let chosen = best([
            Gpu {
                name: "Intel(R) UHD Graphics 630".into(),
                kind: GpuKind::Integrated,
                memory: Some(GIB),
            },
            Gpu {
                name: "NVIDIA GeForce RTX 3060".into(),
                kind: GpuKind::Discrete,
                memory: Some(12 * GIB),
            },
        ]);
        assert_eq!(chosen.unwrap().kind, GpuKind::Discrete);
        assert_eq!(best([]), None);
    }

    #[test]
    fn tiers() {
        // A gaming PC.
        assert_eq!(
            Tier::of(&hw(32, 16, gpu(GpuKind::Discrete, Some(12)))),
            Tier::Ultra
        );
        // The same card with 8 GB of memory.
        assert_eq!(
            Tier::of(&hw(8, 16, gpu(GpuKind::Discrete, Some(12)))),
            Tier::High
        );
        // A mid-range card.
        assert_eq!(
            Tier::of(&hw(16, 8, gpu(GpuKind::Discrete, Some(4)))),
            Tier::High
        );
        assert_eq!(
            Tier::of(&hw(16, 8, gpu(GpuKind::Discrete, Some(2)))),
            Tier::Medium
        );
        // A school laptop.
        assert_eq!(
            Tier::of(&hw(8, 8, gpu(GpuKind::Integrated, Some(1)))),
            Tier::Low
        );
        // Apple silicon.
        assert_eq!(
            Tier::of(&hw(16, 8, gpu(GpuKind::AppleSilicon, None))),
            Tier::High
        );
        assert_eq!(
            Tier::of(&hw(8, 8, gpu(GpuKind::AppleSilicon, None))),
            Tier::Medium
        );
        // Nothing known about the graphics.
        assert_eq!(Tier::of(&hw(16, 8, None)), Tier::Medium);
        assert_eq!(Tier::of(&hw(8, 4, None)), Tier::Low);
        // A slow processor holds back a fast card.
        assert_eq!(
            Tier::of(&hw(16, 4, gpu(GpuKind::Discrete, Some(8)))),
            Tier::Medium
        );
        assert_eq!(
            Tier::of(&hw(4, 8, gpu(GpuKind::Discrete, Some(8)))),
            Tier::Low
        );
    }
}
