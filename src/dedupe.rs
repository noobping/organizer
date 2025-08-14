use crate::actions::{Action, ActionEngine};
use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::hash::Hash;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DedupeMethod {
    Name,
    Size,
    Hash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DedupeMode {
    Delete,
    Hardlink,
    Symlink,
}

#[derive(Debug, Clone)]
struct FileInfo {
    path: PathBuf,
    name: String,
    size: u64,
    hash: Option<[u8; 32]>,
}

impl FileInfo {
    fn compute_hash(&mut self) -> Result<()> {
        if self.hash.is_none() {
            let mut hasher = blake3::Hasher::new();
            let mut f = fs::File::open(&self.path)?;
            std::io::copy(&mut f, &mut hasher)?;
            self.hash = Some(*hasher.finalize().as_bytes());
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct DedupePlan {
    methods: Vec<DedupeMethod>,
    files: Vec<FileInfo>,
}

impl DedupePlan {
    pub fn new(methods: Vec<DedupeMethod>) -> Self {
        Self { methods, files: vec![] }
    }

    pub fn scan(&mut self, root: &Path) -> Result<()> {
        // Collect files recursively
        for entry in walkdir::WalkDir::new(root).follow_links(false) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if entry.file_type().is_file() {
                let path = entry.path().to_path_buf();
                let name = entry.file_name().to_string_lossy().to_string();
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                self.files.push(FileInfo { path, name, size, hash: None });
            }
        }
        // If hash is required, compute in parallel
        if self.methods.contains(&DedupeMethod::Hash) {
            self.files.par_iter_mut().for_each(|f| { let _ = f.compute_hash(); });
        }
        Ok(())
    }

    pub fn apply(&self, mode: DedupeMode, engine: &mut ActionEngine) -> Result<()> {
        // Group by selected key(s)
        let mut groups: HashMap<String, Vec<&FileInfo>> = HashMap::new();
        for fi in &self.files {
            let mut parts: Vec<String> = vec![];
            for m in &self.methods {
                match m {
                    DedupeMethod::Name => parts.push(format!("N:{}", fi.name)),
                    DedupeMethod::Size => parts.push(format!("S:{}", fi.size)),
                    DedupeMethod::Hash => {
                        let hex = fi.hash.map(|h| hex::encode(h)).unwrap_or_else(|| "NOHASH".into());
                        parts.push(format!("H:{}", hex));
                    }
                }
            }
            let key = parts.join("|");
            groups.entry(key).or_default().push(fi);
        }

        // For each group with >1, keep first, remove others
        for (_k, vecf) in groups.into_iter() {
            if vecf.len() <= 1 { continue; }
            // Keep the first file, operate on the rest
            let (keep, rest) = vecf.split_first().unwrap();
            for dup in rest {
                match mode {
                    DedupeMode::Delete => {
                        // current behavior: just delete duplicates
                        engine.execute(&Action::Delete(dup.path.clone(), "duplicate file".into()))?;
                    }
                    DedupeMode::Hardlink => {
                        // replace duplicate with a hardlink to the kept file
                        engine.execute(&Action::Delete(dup.path.clone(), "duplicate file (to hardlink)".into()))?;
                        if engine.apply_mode() {
                            let _ = std::fs::hard_link(&keep.path, &dup.path);
                        }
                    }
                    DedupeMode::Symlink => {
                        // replace duplicate with a symlink to the kept file
                        engine.execute(&Action::Delete(dup.path.clone(), "duplicate file (to symlink)".into()))?;
                        if engine.apply_mode() {
                            #[cfg(unix)]
                            { let _ = std::os::unix::fs::symlink(&keep.path, &dup.path); }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

// local hex encode to avoid extra deps
mod hex {
    pub fn encode(bytes: [u8;32]) -> String {
        let mut s = String::with_capacity(64);
        for b in bytes {
            s.push(hex_char(b >> 4));
            s.push(hex_char(b & 0x0f));
        }
        s
    }
    #[inline]
    fn hex_char(n: u8) -> char {
        match n {
            0..=9 => (b'0' + n) as char,
            10..=15 => (b'a' + (n - 10)) as char,
            _ => '?',
        }
    }
}
