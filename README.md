# Organizer

Rust CLI to **organize, deduplicate, and clean** large data trees.

## ✨ Features

- **Default is DRY-RUN.** Add `--apply` to actually move/delete.
- Categories: `Media, Music, Documents, Archives, Projects, GitRepos, Backups, Others`
- Detects **home-folder backups**, **git repos** (bare and working), and **code projects** to move **as a whole**.
- Uses extension lists with optional content sniffing (`--use-file_cmd` to call `file(1)`; otherwise uses the Rust `infer` crate).
- Removes broken symlinks and known temp/cache files (configurable).
- Optional **duplicate removal** with `--dedup` (`name`, `size`, `hash`, or `all`).

Tested on Fedora Silverblue-style systems (immutable host). Moving uses `rename(2)` where possible, avoiding copies.

## 🚀 Build

```bash
cd organizer
cargo build --release
```

Binary at `target/release/organizer`.

## 📦 Usage Examples

```bash
# Dry run (default), organize current directory into categories at root
organizer .

# Actually apply moves/deletes
organizer --apply /mnt

# Organize under "Organized" inside /mnt
organizer --apply --under Organized /mnt

# Deduplicate by size and hash
organizer --apply --dedup size --dedup hash /mnt

# Deduplicate using all methods
organizer --apply --dedup all /mnt

# Skip cleaning temp/cache files
organizer --apply --no-clean /mnt

# Log all actions to a file
organizer --apply --log-file /mnt/organize.log /mnt
```

## ⚙️ Configuration

On first run, default config files are created at: `~/.config/organizer/`.

| Config File              | Purpose                               |
|--------------------------|---------------------------------------|
| `media_extensions.txt`   | File extensions for Media category    |
| `audio_extensions.txt`   | File extensions for Music category    |
| `document_extensions.txt`| File extensions for Documents         |
| `archive_extensions.txt` | File extensions for Archives          |
| `code_extensions.txt`    | File extensions for Code Projects     |
| `home_markers.txt`       | Patterns for detecting home backups   |
| `delete_patterns.txt`    | Patterns for cleaning temp/cache files|

Each file is a plain list **one item per line**.

## 📌 Notes

- Moves use `rename(2)` — no copies unless crossing filesystems with `--allow-cross-device`.
- Cross-device moves will **copy then delete** (slower, needs space).
- Symlinks are not followed by default.
- Broken symlinks are removed with `--clean`.
- Single code files aren’t treated as projects (avoids scattering).

## 🛠 Design Philosophy

Managing large, disorganized archives is tedious. This tool aims to:

1. **Classify files** into meaningful categories.
2. **Preserve structure** for projects, backups, and special folders.
3. **Clean junk** safely (caches, broken links).
4. **Eliminate duplicates** to save space.
5. Provide **full transparency** with logs and dry-run previews.

More in the [design document](Design.md).

## 🙌 Acknowledgements

Inspired by tools like [`fclones`](https://github.com/pkolaczk/fclones) and [`rmlint`](https://rmlint.readthedocs.io/). 
