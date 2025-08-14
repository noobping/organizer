use anyhow::{Context, Result};
use dirs::config_dir;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Write, BufRead, BufReader};
use std::path::{Path, PathBuf};

pub const APP_DIR: &str = "organizer";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    /// category -> extensions
    pub category_exts: HashMap<String, Vec<String>>,
    /// delete/ignore glob patterns
    pub delete_patterns: Vec<String>,
    /// names used to detect a "home backup" dir
    pub home_markers: Vec<String>,
    /// code project hints: extensions
    pub code_exts: Vec<String>,
}

impl Settings {
    pub fn load_or_default() -> Result<Self> {
        let s = Self::default();
        s.ensure_default_lists_written()?;
        // After writing defaults (if needed), load them from files
        let dir = config_dir().unwrap_or_else(|| PathBuf::from(".")).join(APP_DIR);
        let media = read_lines_into_vec(dir.join("media_extensions.txt")).unwrap_or_else(|_| default_media_exts());
        let audio = read_lines_into_vec(dir.join("audio_extensions.txt")).unwrap_or_else(|_| default_audio_exts());
        let docs  = read_lines_into_vec(dir.join("document_extensions.txt")).unwrap_or_else(|_| default_document_exts());
        let arch  = read_lines_into_vec(dir.join("archive_extensions.txt")).unwrap_or_else(|_| default_archive_exts());
        let code  = read_lines_into_vec(dir.join("code_extensions.txt")).unwrap_or_else(|_| default_code_exts());

        let mut category_exts = HashMap::new();
        category_exts.insert("Media".to_string(), media);
        category_exts.insert("Music".to_string(), audio);
        category_exts.insert("Documents".to_string(), docs);
        category_exts.insert("Archives".to_string(), arch);

        let delete_patterns = read_lines_into_vec(dir.join("delete_patterns.txt")).unwrap_or_else(|_| default_delete_patterns());
        let home_markers = read_lines_into_vec(dir.join("home_markers.txt")).unwrap_or_else(|_| default_home_markers());

        Ok(Self {
            category_exts,
            delete_patterns,
            home_markers,
            code_exts: code,
        })
    }

    pub fn ensure_default_lists_written(&self) -> Result<()> {
        let base = config_dir().unwrap_or_else(|| PathBuf::from(".")).join(APP_DIR);
        fs::create_dir_all(&base).context("create config dir")?;

        write_default_if_missing(base.join("media_extensions.txt"), &default_media_exts())?;
        write_default_if_missing(base.join("audio_extensions.txt"), &default_audio_exts())?;
        write_default_if_missing(base.join("document_extensions.txt"), &default_document_exts())?;
        write_default_if_missing(base.join("archive_extensions.txt"), &default_archive_exts())?;
        write_default_if_missing(base.join("code_extensions.txt"), &default_code_exts())?;
        write_default_if_missing(base.join("home_markers.txt"), &default_home_markers())?;
        write_default_if_missing(base.join("delete_patterns.txt"), &default_delete_patterns())?;
        Ok(())
    }

    pub fn delete_matcher(&self) -> Result<GlobSet> {
        let mut builder = GlobSetBuilder::new();
        for pat in &self.delete_patterns {
            // tolerant: skip malformed
            if let Ok(gl) = Glob::new(pat) {
                builder.add(gl);
            }
        }
        Ok(builder.build()?)
    }

    pub fn category_names(&self) -> Vec<String> {
        vec!["Media","Music","Documents","Archives","Projects","GitRepos","Backups","Others"]
            .into_iter().map(|s| s.to_string()).collect()
    }
}

/// Helpers

fn write_default_if_missing(path: PathBuf, lines: &Vec<String>) -> Result<()> {
    if !path.exists() {
        let mut f = fs::File::create(&path)?;
        for l in lines {
            if !l.trim().is_empty() {
                writeln!(f, "{l}")?;
            }
        }
    }
    Ok(())
}

fn read_lines_into_vec<P: AsRef<Path>>(p: P) -> Result<Vec<String>> {
    let f = fs::File::open(&p)?;
    let br = BufReader::new(f);
    let mut v = vec![];
    for line in br.lines() {
        let l = line?.trim().to_string();
        if l.is_empty() || l.starts_with('#') { continue; }
        v.push(l);
    }
    Ok(v)
}

fn default_media_exts() -> Vec<String> {
    vec!["jpg","jpeg","png","gif","bmp","tiff","tif","webp","heic","heif","raw","cr2","nef","arw","raf","dng",
         "mp4","mkv","avi","mov","flv","webm","mpeg","mpg","m4v","3gp","3g2"]
        .into_iter().map(|s| s.to_string()).collect()
}
fn default_audio_exts() -> Vec<String> {
    vec!["mp3","wav","flac","ogg","oga","opus","aac","m4a","wma","aiff","aif","mid","midi"]
        .into_iter().map(|s| s.to_string()).collect()
}
fn default_document_exts() -> Vec<String> {
    vec!["pdf","doc","docx","xls","xlsx","ppt","pptx","odt","ods","odp","rtf","txt","csv","md","markdown","epub","mobi"]
        .into_iter().map(|s| s.to_string()).collect()
}
fn default_archive_exts() -> Vec<String> {
    vec!["zip","tar","gz","tgz","bz2","tbz","xz","7z","rar","iso","img"]
        .into_iter().map(|s| s.to_string()).collect()
}
fn default_code_exts() -> Vec<String> {
    vec!["rs","py","c","cpp","h","hpp","java","kt","go","js","ts","tsx","jsx","php","rb","swift","cs","sh","bash","zsh","fish","ps1","pl","lua","r","sql","json","toml","yaml","yml","xml","gradle","lock","makefile","cmake"]
        .into_iter().map(|s| s.to_string()).collect()
}
fn default_home_markers() -> Vec<String> {
    vec![
        "Documents","Documenten","Downloads","Afbeeldingen","Pictures","Muziek","Music","Videos","Video's","Bureaublad","Desktop",
        ".config",".local",".bash_history",".bashrc",".profile","Public","Publiek"
    ].into_iter().map(|s| s.to_string()).collect()
}
fn default_delete_patterns() -> Vec<String> {
    vec![
        "**/.cache/**",
        "**/Cache/**",
        "**/Thumbnails/**",
        "**/thumbs.db",
        "**/Thumbs.db",
        "**/.DS_Store",
        "**/*.tmp",
        "**/*.temp",
        "**/*.swp",
        "**/*.swo",
        "**/*~",
        "**/.Trash/**",
        "**/*.part",
        "**/~$*",
        "**/desktop.ini",
    ].into_iter().map(|s| s.to_string()).collect()
}
