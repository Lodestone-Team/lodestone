//! The metadata inside mod and plugin jars, read by [`lodestone_minecraft::jar`].

pub use lodestone_minecraft::jar::*;

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Write;
    use std::path::Path;

    /// Writes a zip with the given entries.
    pub(crate) fn make_zip(path: &Path, entries: &[(&str, &str)]) {
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for (name, contents) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(contents.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }
}
