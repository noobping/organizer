use anyhow::{Context, Result};
use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum Action {
    MoveFile(PathBuf, PathBuf), // src, dest_dir
    MoveDir(PathBuf, PathBuf),  // src_dir, dest_dir
    Delete(PathBuf, String),    // path, reason
}

pub struct ActionEngine {
    apply: bool,
    allow_cross_device: bool,
    log_file: Option<std::fs::File>,
}

impl ActionEngine {
    pub fn new(apply: bool, allow_cross_device: bool, log_path: Option<&PathBuf>) -> Result<Self> {
        let log_file = if let Some(p) = log_path {
            Some(std::fs::OpenOptions::new().create(true).append(true).open(p)?)
        } else { None };
        Ok(Self { apply, allow_cross_device, log_file })
    }

    pub fn apply_mode(&self) -> bool { self.apply }

    pub fn execute_all(&mut self, actions: &[Action]) -> Result<()> {
        for a in actions {
            self.execute(a)?;
        }
        Ok(())
    }

    pub fn execute(&mut self, action: &Action) -> Result<()> {
        match action {
            Action::MoveFile(src, dest_dir) => self.move_file(src, dest_dir),
            Action::MoveDir(src_dir, dest_dir) => self.move_dir(src_dir, dest_dir),
            Action::Delete(path, reason) => self.delete(path, reason),
        }
    }

    pub fn prune_empty_dirs(&mut self, root: &Path, skip_roots: &std::collections::HashSet<PathBuf>) -> Result<()> {
        // Walk bottom-up to remove empties
        for entry in walkdir::WalkDir::new(root).min_depth(1).max_depth(usize::MAX).contents_first(true) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path().to_path_buf();
            if skip_roots.iter().any(|p| path.starts_with(p)) {
                continue;
            }
            if entry.file_type().is_dir() {
                if is_dir_empty(&path)? {
                    self.log(format!("PRUNE {}", display(&path)));
                    if self.apply {
                        let _ = fs::remove_dir(&path);
                    }
                }
            }
        }
        Ok(())
    }

    fn move_file(&mut self, src: &Path, dest_dir: &Path) -> Result<()> {
        let file_name = src.file_name().unwrap_or_default();
        let mut dest_path = dest_dir.join(file_name);

        // Ensure dest dir exists
        self.log(format!("MOVE {} -> {}", display(src), display(&dest_path)));
        if self.apply {
            fs::create_dir_all(dest_dir).context("create dest dir")?;
            dest_path = unique_dest_path(&dest_path);
            match fs::rename(src, &dest_path) {
                Ok(_) => {}
                Err(err) if is_cross_device(&err) && self.allow_cross_device => {
                    // Fallback to copy+remove (can be expensive on nearly full disks)
                    fs::copy(src, &dest_path).context("copy across device")?;
                    fs::remove_file(src).ok();
                }
                Err(err) => {
                    self.log(format!("ERROR moving {}: {}", display(src), err));
                }
            }
        }
        Ok(())
    }

    fn move_dir(&mut self, src_dir: &Path, dest_dir: &Path) -> Result<()> {
        let mut dest = dest_dir.to_path_buf();
        self.log(format!("MOVE-DIR {} -> {}", display(src_dir), display(&dest)));
        if self.apply {
            // Append suffix if dest exists
            if dest.exists() {
                dest = unique_dir_dest(&dest);
            }
            // Try rename first
            match fs::rename(src_dir, &dest) {
                Ok(_) => {}
                Err(err) if is_cross_device(&err) && self.allow_cross_device => {
                    // Cross device dir move: copy recursively then remove
                    copy_dir_recursive(src_dir, &dest)?;
                    let _ = fs::remove_dir_all(src_dir);
                }
                Err(err) => {
                    self.log(format!("ERROR moving dir {}: {}", display(src_dir), err));
                }
            }
        }
        Ok(())
    }

    fn delete(&mut self, path: &Path, reason: &str) -> Result<()> {
        self.log(format!("DELETE {} ({})", display(path), reason));
        if self.apply {
            if path.is_dir() {
                let _ = fs::remove_dir_all(path);
            } else {
                let _ = fs::remove_file(path);
            }
        }
        Ok(())
    }

    fn log(&mut self, line: String) {
        println!("{}", line);
        if let Some(f) = self.log_file.as_mut() {
            let _ = writeln!(f, "{}", line);
        }
    }
}

/// Helpers

fn display(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn is_cross_device(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::Other && format!("{}", err).contains("EXDEV")
}

fn unique_dest_path(path: &Path) -> PathBuf {
    if !path.exists() { return path.to_path_buf(); }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    for i in 1..10000 {
        let candidate = if ext.is_empty() {
            parent.join(format!("{}-{}", stem, i))
        } else {
            parent.join(format!("{}-{}.{}", stem, i, ext))
        };
        if !candidate.exists() { return candidate; }
    }
    path.to_path_buf()
}

fn unique_dir_dest(path: &Path) -> PathBuf {
    if !path.exists() { return path.to_path_buf(); }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("dir");
    for i in 1..10000 {
        let candidate = parent.join(format!("{}-{}", name, i));
        if !candidate.exists() { return candidate; }
    }
    path.to_path_buf()
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in walkdir::WalkDir::new(src).min_depth(1) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src).unwrap();
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(p) = target.parent() { std::fs::create_dir_all(p)?; }
            std::fs::copy(entry.path(), &target)?;
        } else if entry.file_type().is_symlink() {
            // replicate symlink where possible
            if let Ok(target_link) = std::fs::read_link(entry.path()) {
                #[cfg(unix)]
                std::os::unix::fs::symlink(&target_link, &target).ok();
            }
        }
    }
    Ok(())
}

fn is_dir_empty(dir: &Path) -> Result<bool> {
    for e in std::fs::read_dir(dir)? {
        let _ = e?;
        return Ok(false);
    }
    Ok(true)
}
