use std::fs::Metadata;

/// Returns true when a file has multiple hard links.
///
/// Multiple links can allow path-based workspace guards to be bypassed by
/// linking a workspace path to external sensitive content.
pub fn has_multiple_hard_links(metadata: &Metadata) -> bool {
    link_count(metadata) > 1
}

#[cfg(unix)]
fn link_count(metadata: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    metadata.nlink()
}

#[cfg(windows)]
fn link_count(_metadata: &Metadata) -> u64 {
    1
}

#[cfg(not(any(unix, windows)))]
fn link_count(_metadata: &Metadata) -> u64 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_link_file_is_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("single.txt");
        std::fs::write(&file, "hello").unwrap();
        let meta = std::fs::metadata(&file).unwrap();
        assert!(!has_multiple_hard_links(&meta));
    }
}
