mod config;
mod categorize;
mod dedupe;
mod actions;
mod utils;

use crate::categorize::Categorizer;
use crate::config::Settings;
use crate::actions::{Action, ActionEngine};
use crate::dedupe::{DedupeMethod, DedupePlan, DedupeMode};
use crate::utils::{is_broken_symlink, is_pattern_match, readable_display};
use anyhow::Result;
use clap::{ArgAction, Parser, ValueEnum};
use walkdir::WalkDir;
use std::collections::HashSet;
use std::path::PathBuf;
use time::macros::format_description;
use time::OffsetDateTime;

/// CLI args
#[derive(Parser, Debug)]
#[command(name="organizer", version, about="Organize, deduplicate, and clean huge folders (dry-run by default).")]
struct Cli {
    /// Root path to organize. Defaults to current directory.
    #[arg(value_name="PATH", default_value=".")]
    root: PathBuf,

    /// Apply changes (move/delete). By default, it's a dry run.
    #[arg(long, action=ArgAction::SetTrue)]
    apply: bool,

    /// Create categories under a single folder name (e.g. 'Organized').
    /// If omitted, categories are created directly in the root.
    #[arg(long, value_name="NAME")]
    under: Option<String>,

    /// Enable duplicate handling. May be given multiple times: --dedup name --dedup size --dedup hash
    /// Or use --dedup all
    #[arg(long, value_enum)]
    dedup: Vec<DedupArg>,

    /// What to do with duplicates: delete (default), hardlink, or symlink
    #[arg(long, value_enum, default_value_t=DedupModeArg::Delete)]
    dedup_mode: DedupModeArg,

    /// Remove known cache/temp files and broken symlinks
    #[arg(long, default_value_t=true, action=ArgAction::Set)]
    clean: bool,

    /// Remove empty directories after moving
    #[arg(long, default_value_t=true, action=ArgAction::Set)]
    prune_empty_dirs: bool,

    /// Follow symlinks when scanning (dangerous). Default: do not follow.
    #[arg(long, default_value_t=false, action=ArgAction::Set)]
    follow_symlinks: bool,

    /// Use `file -b --mime-type` for content detection when extension is unknown (falls back to `infer` crate).
    #[arg(long, default_value_t=false, action=ArgAction::Set)]
    use_file_cmd: bool,

    /// Allow cross-device moves by copy+delete if rename fails with EXDEV.
    #[arg(long, default_value_t=false, action=ArgAction::Set)]
    allow_cross_device: bool,

    /// Log file to append detailed actions (in addition to stdout).
    #[arg(long, value_name="FILE")]
    log_file: Option<PathBuf>,

    /// Skip creating default config files if missing
    #[arg(long, default_value_t=false, action=ArgAction::Set)]
    no_write_defaults: bool,
}

#[derive(Clone, Debug, ValueEnum)]
enum DedupArg {
    All,
    Name,
    Size,
    Hash,
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum DedupModeArg {
    Delete,
    Hardlink,
    Symlink,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Timestamp header
    let now = OffsetDateTime::now_utc();
    let fmt = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    println!("# organizer @ {}", now.format(fmt).unwrap_or_default());
    println!("# Root: {}", readable_display(&cli.root));
    println!("# Mode: {}", if cli.apply { "APPLY (will change files!)" } else { "DRY-RUN (no changes)" });

    // Load settings + ensure default config files exist
    let settings = Settings::load_or_default()?;
    if !cli.no_write_defaults {
        settings.ensure_default_lists_written()?;
    }

    // Categorizer
    let categorizer = Categorizer::new(&settings, cli.use_file_cmd)?;

    // Calculate destination root (maybe under an "Organized" folder or directly in root)
    let dest_root = if let Some(name) = &cli.under {
        cli.root.join(name)
    } else {
        cli.root.clone()
    };

    // Ensure category directories exist in DRY-RUN? We will create only in APPLY phase.
    let mut action_engine = ActionEngine::new(cli.apply, cli.allow_cross_device, cli.log_file.as_ref())?;

    // Build ignore matcher for delete patterns and avoid scanning our destination categories
    let delete_matcher = settings.delete_matcher()?;
    let category_dirs: HashSet<String> = settings.category_names().into_iter().collect();
    let mut skip_dirs: HashSet<PathBuf> = HashSet::new();
    // Skip destination categories already present
    for cat in &category_dirs {
        skip_dirs.insert(dest_root.join(cat));
    }
    if let Some(name) = &cli.under {
        skip_dirs.insert(cli.root.join(name));
    }

    // Walk the tree using ignore::WalkBuilder (respects .gitignore, can follow symlinks optional)
    let mut it = WalkDir::new(&cli.root).follow_links(cli.follow_symlinks).into_iter();

    // To avoid recursing into directories we've decided to move as a whole
    let mut planned_whole_dirs: HashSet<PathBuf> = HashSet::new();

    // Collect actions first
    let mut planned_actions: Vec<Action> = Vec::new();

    while let Some(res) = it.next() {
        let dent = match res {
            Ok(d) => d,
            Err(err) => {
                println!("WARN: skipping entry due to error: {err}");
                continue;
            }
        };

        let path = dent.path().to_path_buf();

        // Skip the root itself in decisions; also skip destination categories and organized root
        if skip_dirs.iter().any(|p| path.starts_with(p)) {
            if dent.file_type().is_dir() {
                it.skip_current_dir();
            }
            continue;
        }

        // If any ancestor is a planned whole-dir move, skip its contents
        if planned_whole_dirs.iter().any(|ancestor| path.starts_with(ancestor)) {
            if dent.file_type().is_dir() {
                it.skip_current_dir();
            }
            continue;
        }

        // Handle symlinks (broken)
        if dent.file_type().is_symlink() {
            if is_broken_symlink(&path) {
                planned_actions.push(Action::Delete(path.clone(), "broken symlink".into()));
            }
            continue;
        }

        // If directory: check for special directories to move as whole
        if dent.file_type().is_dir() {
            // Is this a category dir already? Skip
            let name = dent.file_name().to_string_lossy().to_string();
            if category_dirs.contains(&name) || (Some(&name) == cli.under.as_ref()) {
                it.skip_current_dir();
                continue;
            }

            // Detect special: backup/home, project (.git), bare git repo
            if let Some(dir_cat) = categorizer.detect_special_directory(&path) {
                let dest_dir = dest_root.join(dir_cat.as_dir()).join(path.file_name().unwrap_or_default());
                planned_actions.push(Action::MoveDir(path.clone(), dest_dir));
                planned_whole_dirs.insert(path.clone());
                it.skip_current_dir();
                continue;
            }

            // Else keep walking inside
            continue;
        }

        // Handle files: delete patterns?
        if is_pattern_match(&delete_matcher, &path) && cli.clean {
            planned_actions.push(Action::Delete(path.clone(), "cache/temp/junk (pattern)".into()));
            continue;
        }

        // Empty files?
        if dent.metadata().map(|m| m.len() == 0).unwrap_or(false) && cli.clean {
            planned_actions.push(Action::Delete(path.clone(), "empty file".into()));
            continue;
        }

        // Categorize file and plan move
        let category = categorizer.categorize_file(&path)?;
        let dest_dir = dest_root.join(category.as_dir());
        planned_actions.push(Action::MoveFile(path.clone(), dest_dir));
    }

    // Execute planned moves/deletions
    action_engine.execute_all(&planned_actions)?;

    // Optionally prune empty directories (post-move)
    if cli.prune_empty_dirs {
        action_engine.prune_empty_dirs(&cli.root, &skip_dirs)?;
    }

    // Dedupe phase
    let dedup_methods: Vec<DedupeMethod> = if cli.dedup.is_empty() {
        vec![]
    } else if cli.dedup.iter().any(|d| matches!(d, DedupArg::All)) {
        vec![DedupeMethod::Name, DedupeMethod::Size, DedupeMethod::Hash]
    } else {
        cli.dedup.iter().map(|d| match d {
            DedupArg::Name => DedupeMethod::Name,
            DedupArg::Size => DedupeMethod::Size,
            DedupArg::Hash => DedupeMethod::Hash,
            DedupArg::All => unreachable!(),
        }).collect()
    };

    if !dedup_methods.is_empty() {
        println!("# DEDUPE with methods: {:?}", dedup_methods);
        let mode = match cli.dedup_mode {
            DedupModeArg::Delete => DedupeMode::Delete,
            DedupModeArg::Hardlink => DedupeMode::Hardlink,
            DedupModeArg::Symlink => DedupeMode::Symlink,
        };
        let mut plan = DedupePlan::new(dedup_methods);
        plan.scan(&dest_root)?;
        plan.apply(mode, &mut action_engine)?;
    }

    println!("# DONE. {} actions planned{}.",
        planned_actions.len(),
        if action_engine.apply_mode() { " and executed" } else { " (dry-run only)" }
    );

    Ok(())
}
