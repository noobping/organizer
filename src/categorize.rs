use crate::config::Settings;
use anyhow::Result;
use infer;
use std::fs;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Media,
    Music,
    Documents,
    Archives,
    Projects,
    GitRepos,
    Backups,
    Others,
}

impl Category {
    pub fn as_dir(&self) -> &'static str {
        match self {
            Category::Media => "Media",
            Category::Music => "Music",
            Category::Documents => "Documents",
            Category::Archives => "Archives",
            Category::Projects => "Projects",
            Category::GitRepos => "GitRepos",
            Category::Backups => "Backups",
            Category::Others => "Others",
        }
    }
}

pub struct Categorizer<'a> {
    settings: &'a Settings,
    use_file_cmd: bool,
}

impl<'a> Categorizer<'a> {
    pub fn new(settings: &'a Settings, use_file_cmd: bool) -> Result<Self> {
        Ok(Self { settings, use_file_cmd })
    }

    pub fn categorize_file(&self, path: &Path) -> Result<Category> {
        let ext = path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase());

        if let Some(ext) = ext {
            if self.settings.category_exts.get("Media").map_or(false, |v| v.iter().any(|e| e == &ext)) {
                return Ok(Category::Media);
            }
            if self.settings.category_exts.get("Music").map_or(false, |v| v.iter().any(|e| e == &ext)) {
                return Ok(Category::Music);
            }
            if self.settings.category_exts.get("Documents").map_or(false, |v| v.iter().any(|e| e == &ext)) {
                return Ok(Category::Documents);
            }
            if self.settings.category_exts.get("Archives").map_or(false, |v| v.iter().any(|e| e == &ext)) {
                return Ok(Category::Archives);
            }
            // Code files fall under Projects ONLY when it's a dir; single code files go to Others unless desired otherwise.
        }

        // Try MIME detection by content for ambiguous files
        if self.use_file_cmd {
            if let Some(mime) = mime_via_file_cmd(path) {
                if mime.starts_with("image/") || mime.starts_with("video/") {
                    return Ok(Category::Media);
                } else if mime.starts_with("audio/") {
                    return Ok(Category::Music);
                } else if is_document_mime(&mime) {
                    return Ok(Category::Documents);
                } else if is_archive_mime(&mime) {
                    return Ok(Category::Archives);
                }
            }
        }
        // fallback to infer
        if let Ok(mut f) = fs::File::open(path) {
            let mut buf = [0u8; 8192];
            let n = f.read(&mut buf).unwrap_or(0);
            let slice = &buf[..n];
            if let Some(kind) = infer::get(slice) {
                let mime = kind.mime_type();
                if mime.starts_with("image/") || mime.starts_with("video/") {
                    return Ok(Category::Media);
                } else if mime.starts_with("audio/") {
                    return Ok(Category::Music);
                }
            }
        }

        Ok(Category::Others)
    }

    /// Detect special directories to be moved as a whole: Backups (home), Projects (.git), bare Git repos.
    pub fn detect_special_directory(&self, dir: &Path) -> Option<Category> {
        // Bare git repo?
        if is_bare_git_repo(dir) {
            return Some(Category::GitRepos);
        }
        // Git working repo?
        if dir.join(".git").is_dir() {
            return Some(Category::Projects);
        }
        // Heuristic: many code files?
        let mut code_count = 0usize;
        if let Ok(rd) = dir.read_dir() {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_file() {
                    if let Some(ext) = p.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                        if self.settings.code_exts.iter().any(|e| e == &ext) {
                            code_count += 1;
                            if code_count >= 5 {
                                return Some(Category::Projects);
                            }
                        }
                    }
                }
            }
        }

        // Home backup detection: look for marker names
        let mut markers_found = 0usize;
        if let Ok(rd) = dir.read_dir() {
            for entry in rd.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if self.settings.home_markers.iter().any(|m| m == &name) {
                    markers_found += 1;
                }
            }
        }
        if markers_found >= 3 {
            return Some(Category::Backups);
        }

        None
    }
}

/// Helpers

fn mime_via_file_cmd(path: &Path) -> Option<String> {
    use std::process::Command;
    let out = Command::new("file")
        .arg("-b")
        .arg("--mime-type")
        .arg(path)
        .output()
        .ok()?;
    if !out.status.success() { return None; }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn is_document_mime(m: &str) -> bool {
    m == "application/pdf" ||
    m == "application/msword" ||
    m == "application/vnd.openxmlformats-officedocument.wordprocessingml.document" ||
    m == "application/vnd.ms-excel" ||
    m == "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" ||
    m == "application/vnd.ms-powerpoint" ||
    m == "application/vnd.openxmlformats-officedocument.presentationml.presentation" ||
    m.starts_with("text/")
}

fn is_archive_mime(m: &str) -> bool {
    m == "application/zip" ||
    m == "application/x-tar" ||
    m == "application/gzip" ||
    m == "application/x-7z-compressed" ||
    m == "application/x-rar-compressed" ||
    m == "application/x-xz"
}

fn is_bare_git_repo(dir: &Path) -> bool {
    if !dir.is_dir() { return false; }
    let name = dir.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if name.ends_with(".git") && dir.join("config").is_file() && dir.join("objects").is_dir() && dir.join("refs").is_dir() {
        return true;
    }
    // Also check config contains "bare = true"
    let cfg = dir.join("config");
    if cfg.is_file() {
        if let Ok(s) = std::fs::read_to_string(cfg) {
            if s.lines().any(|l| l.trim() == "bare = true") {
                return true;
            }
        }
    }
    false
}
