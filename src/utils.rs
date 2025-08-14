use globset::GlobSet;
use std::path::Path;

pub fn is_broken_symlink(path: &Path) -> bool {
    if let Ok(md) = std::fs::symlink_metadata(path) {
        if md.file_type().is_symlink() {
            return std::fs::read_link(path).map(|target| !target.exists()).unwrap_or(true);
        }
    }
    false
}

pub fn is_pattern_match(matcher: &GlobSet, path: &Path) -> bool {
    matcher.is_match(path)
}

pub fn readable_display(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}
