//! Font files in skin packs: checked to be TrueType or OpenType fonts of a sane size
//! before they are stored, and their family name read so the launcher can ask for
//! them by name. Fonts are data (outlines and tables), nothing that runs; the
//! launcher hands them to the system's text engine like the fonts it ships.

use super::SkinError;

/// The largest font file a pack may hold.
pub const MAX_FONT_BYTES: usize = 2 * 1024 * 1024;
/// The most font files in a pack.
pub const MAX_FONT_FILES: usize = 4;

/// Whether `name` may name a font file in the pack: a plain `.ttf` or `.otf` name.
pub fn check_font_name(name: &str) -> Result<(), SkinError> {
    let plain = !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':'])
        && !name.contains("..")
        && !name.chars().any(char::is_control);
    if plain && super::surface::is_font_file(name) {
        Ok(())
    } else {
        Err(SkinError::Invalid(format!(
            "{name:?} must be a .ttf or .otf file in the pack"
        )))
    }
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// Checks that `bytes` are a single TrueType or OpenType font with its tables inside
/// the file, and returns its family name.
pub fn font_family(bytes: &[u8]) -> Result<String, SkinError> {
    let bad = |reason: &str| SkinError::Invalid(format!("not a usable font: {reason}"));
    if bytes.len() > MAX_FONT_BYTES {
        return Err(SkinError::Invalid(format!(
            "a font file may be at most {} MB",
            MAX_FONT_BYTES / 1024 / 1024
        )));
    }
    let tag = u32_at(bytes, 0).ok_or_else(|| bad("too short"))?;
    // TrueType (1.0 or 'true') or OpenType with CFF outlines ('OTTO'); no
    // collections, no web fonts
    if !matches!(tag, 0x0001_0000 | 0x7472_7565 | 0x4F54_544F) {
        return Err(bad("it isn't a TrueType or OpenType font"));
    }
    let tables = usize::from(u16_at(bytes, 4).ok_or_else(|| bad("too short"))?);
    if tables == 0 || tables > 64 {
        return Err(bad("its table directory is damaged"));
    }
    let mut name_table = None;
    let mut has = [false; 3]; // cmap, head, hhea
    for i in 0..tables {
        let record = 12 + i * 16;
        let tag = bytes
            .get(record..record + 4)
            .ok_or_else(|| bad("its table directory is cut short"))?;
        let offset = u32_at(bytes, record + 8).ok_or_else(|| bad("cut short"))? as usize;
        let length = u32_at(bytes, record + 12).ok_or_else(|| bad("cut short"))? as usize;
        if offset
            .checked_add(length)
            .is_none_or(|end| end > bytes.len())
        {
            return Err(bad("a table points outside the file"));
        }
        match tag {
            b"name" => name_table = Some((offset, length)),
            b"cmap" => has[0] = true,
            b"head" => has[1] = true,
            b"hhea" => has[2] = true,
            _ => {}
        }
    }
    if has.contains(&false) {
        return Err(bad("it is missing a required table"));
    }
    let (offset, length) = name_table.ok_or_else(|| bad("it has no names"))?;
    let table = &bytes[offset..offset + length];
    let count = usize::from(u16_at(table, 2).ok_or_else(|| bad("its names are damaged"))?);
    let strings = usize::from(u16_at(table, 4).ok_or_else(|| bad("its names are damaged"))?);
    // the typographic family (16) wins over the family (1); Windows Unicode names
    // win over Mac Roman ones
    let mut best: Option<(u8, String)> = None;
    // the system's text engine needs a PostScript name and a full name
    let (mut postscript, mut full) = (false, false);
    for i in 0..count.min(512) {
        let record = 6 + i * 12;
        let (Some(platform), Some(encoding), Some(id), Some(len), Some(at)) = (
            u16_at(table, record),
            u16_at(table, record + 2),
            u16_at(table, record + 6),
            u16_at(table, record + 8),
            u16_at(table, record + 10),
        ) else {
            break;
        };
        let start = strings + usize::from(at);
        let Some(raw) = table.get(start..start + usize::from(len)) else {
            continue;
        };
        let text = match (platform, encoding) {
            (3, 1) | (0, _) => String::from_utf16(
                &raw.as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_be_bytes(*c))
                    .collect::<Vec<_>>(),
            )
            .ok(),
            (1, 0) => Some(raw.iter().map(|&b| char::from(b)).collect()),
            _ => None,
        };
        let Some(text) = text.filter(|t| !t.trim().is_empty() && t.len() <= 64) else {
            continue;
        };
        postscript |= id == 6;
        full |= id == 4;
        let rank = match (id, platform) {
            (16, 3 | 0) => 4,
            (16, _) => 3,
            (1, 3 | 0) => 2,
            (1, _) => 1,
            _ => continue,
        };
        if best.as_ref().is_none_or(|(r, _)| rank > *r) {
            best = Some((rank, text.trim().to_string()));
        }
    }
    if !postscript || !full {
        return Err(bad("it has no PostScript or full name"));
    }
    best.map(|(_, name)| name)
        .filter(|name| !name.chars().any(char::is_control))
        .ok_or_else(|| bad("it has no family name"))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A minimal font: the table directory, empty required tables, and a name table
    /// naming `family`.
    pub fn tiny_font(family: &str) -> Vec<u8> {
        let name: Vec<u8> = family.encode_utf16().flat_map(u16::to_be_bytes).collect();
        let mut name_table = Vec::new();
        name_table.extend(0u16.to_be_bytes()); // format
        name_table.extend(3u16.to_be_bytes()); // count
        name_table.extend(42u16.to_be_bytes()); // strings at: 6 + 3 × 12
        // family, full and PostScript names, all the family's text
        for id in [1u16, 4, 6] {
            for v in [3u16, 1, 0x409, id, name.len() as u16, 0] {
                name_table.extend(v.to_be_bytes());
            }
        }
        name_table.extend(&name);
        let tables: [(&[u8; 4], Vec<u8>); 4] = [
            (b"cmap", vec![0; 4]),
            (b"head", vec![0; 4]),
            (b"hhea", vec![0; 4]),
            (b"name", name_table),
        ];
        let mut out = Vec::new();
        out.extend(0x0001_0000u32.to_be_bytes());
        out.extend((tables.len() as u16).to_be_bytes());
        out.extend([0u8; 6]);
        let mut offset = 12 + tables.len() * 16;
        let mut data: Vec<u8> = Vec::new();
        for (tag, body) in &tables {
            out.extend(*tag);
            out.extend(0u32.to_be_bytes());
            out.extend((offset as u32).to_be_bytes());
            out.extend((body.len() as u32).to_be_bytes());
            offset += body.len();
            data.extend(body);
        }
        out.extend(data);
        out
    }

    #[test]
    fn reads_a_fonts_family_and_refuses_what_isnt_one() {
        assert_eq!(font_family(&tiny_font("Pack Serif")).unwrap(), "Pack Serif");
        assert!(font_family(b"<svg>not a font</svg>").is_err());
        // without a PostScript name the system's text engine would refuse it
        let mut nameless = tiny_font("Pack Serif");
        let at = nameless.windows(4).position(|w| w == b"name").unwrap();
        let table = u32::from_be_bytes(nameless[at + 8..at + 12].try_into().unwrap()) as usize;
        // turn the PostScript record (the third) into a name nobody reads
        nameless[table + 6 + 2 * 12 + 6..table + 6 + 2 * 12 + 8]
            .copy_from_slice(&99u16.to_be_bytes());
        assert!(font_family(&nameless).is_err());
        let mut damaged = tiny_font("X");
        damaged.truncate(40);
        assert!(font_family(&damaged).is_err());
        assert!(font_family(&vec![0; MAX_FONT_BYTES + 1]).is_err());
        assert!(check_font_name("pixel.ttf").is_ok());
        assert!(check_font_name("../pixel.ttf").is_err());
        assert!(check_font_name("pixel.woff2").is_err());
    }

    #[test]
    fn the_bundled_fonts_have_the_names_skins_use() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
        let mut families = std::collections::BTreeSet::new();
        for entry in walk(&dir) {
            if super::super::surface::is_font_file(&entry.to_string_lossy()) {
                families.insert(font_family(&std::fs::read(&entry).unwrap()).unwrap());
            }
        }
        for family in super::super::surface::BUNDLED_FONTS {
            assert!(families.contains(family), "{family} in {families:?}");
        }
    }

    fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walk(&path));
            } else {
                out.push(path);
            }
        }
        out
    }
}
