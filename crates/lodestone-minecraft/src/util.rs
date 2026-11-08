//! Small formatting helpers.

const BYTE_UNITS: [&str; 7] = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];

/// Picks the largest binary unit (1 KB = 1024 B) that keeps `bytes` at 1 or more,
/// and returns its index in `BYTE_UNITS` and its size in bytes.
fn byte_unit(bytes: u64) -> (usize, f64) {
    let mut index = 0;
    let mut size = 1_u64;
    while index + 1 < BYTE_UNITS.len() && bytes / size >= 1024 {
        index += 1;
        size *= 1024;
    }
    (index, size as f64)
}

/// Formats download progress such as `1.5 / 10.0 MB`. The unit is picked from the larger
/// of the two values so both numbers share it.
pub fn format_byte_download(bytes: u64, total: u64) -> String {
    let (index, size) = byte_unit(bytes.max(total));
    if index == 0 {
        format!("{} / {} B", bytes, total)
    } else {
        format!(
            "{:.1} / {:.1} {}",
            bytes as f64 / size,
            total as f64 / size,
            BYTE_UNITS[index]
        )
    }
}

/// Formats a byte count such as `512 B` or `1.5 MB`.
pub fn format_byte(bytes: u64) -> String {
    let (index, size) = byte_unit(bytes);
    if index == 0 {
        format!("{} B", bytes)
    } else {
        format!("{:.1} {}", bytes as f64 / size, BYTE_UNITS[index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_byte() {
        assert_eq!(format_byte(0), "0 B");
        assert_eq!(format_byte(1023), "1023 B");
        assert_eq!(format_byte(1024), "1.0 KB");
        assert_eq!(format_byte(1536), "1.5 KB");
        assert_eq!(format_byte(5 * 1024 * 1024 + 512 * 1024), "5.5 MB");
        assert_eq!(format_byte(u64::MAX), "16.0 EB");

        assert_eq!(format_byte_download(10, 100), "10 / 100 B");
        assert_eq!(format_byte_download(512, 2048), "0.5 / 2.0 KB");
        assert_eq!(
            format_byte_download(1024 * 1024, 3 * 1024 * 1024 / 2),
            "1.0 / 1.5 MB"
        );
        // a download that grows past the reported total still formats
        assert_eq!(format_byte_download(2048, 1024), "2.0 / 1.0 KB");
    }
}
