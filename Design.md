# Design Philosophy

Designing a Rust CLI Tool to Organize and Deduplicate Files.

## Overview

Managing a large, unorganized data archive can be daunting, especially when many files are duplicated or scattered across deep subfolders. We propose a **Rust-based command-line tool** to reorganize files into categorical folders (e.g. Media, Documents, Music, Code Projects, Backups) and eliminate clutter. This tool will **move** files (not copy) to conserve space, remove broken or cache files, and optionally detect duplicate files to save storage. It will default to operating on the current directory (or a specified path) and create an organized folder structure at that root. All operations will be **logged** for transparency, and a **dry-run mode** will allow previewing changes without actually modifying files. The design emphasizes configurability (sensible defaults with user override) and uses file metadata or content to classify files reliably.

## File Classification by Type

To sort files by category, the tool will classify files primarily by **file type**. This can be determined via file extensions and verified by content (using Linux’s file type detection). Linux provides the file command, which ignores file extensions and uses a _magic database_ to identify types by content[\[1\]](https://www.hostinger.com/tutorials/linux-file-command#:~:text=When%20executed%2C%20the%20command%20doesn%E2%80%99t,to%20determine%20the%20file%20type). We can leverage a Rust crate (e.g. binding to libmagic or a crate like infer) to get MIME types from file content as needed. For efficiency, the tool will first use known file extension mappings to guess categories, then fall back to content analysis if the extension is missing or ambiguous. (Linux systems often maintain a mapping of extensions to MIME types, for example in **/etc/mime.types** on Debian/Ubuntu[\[2\]](https://stackoverflow.com/questions/1735659/list-of-all-mimetypes-on-the-planet-mapped-to-file-extensions#:~:text=9).) We will ship the tool with internal default lists of extensions per category, and also create configurable text files for these lists (see **Configurability** below).

**Default Categories and Extensions:** Based on common usage, we define categories and typical file extensions for each:

- **Media (Photos & Videos):** Image formats like .jpg, .png, .gif, .bmp, .tiff, .webp and video formats like .mp4, .mkv, .avi, .mov, .flv, .webm[\[3\]](https://github.com/Joeljaison391/Downloads-Organizer#:~:text=1.%20Images%3A%20,5.%20Audio). (We may subdivide into Images/ and Videos/ subfolders, or keep one combined **Media** folder as per user preference.)
- **Music/Audio:** Audio formats such as .mp3, .wav, .flac, .ogg, .aac, .wma, .m4a[\[4\]](https://github.com/Joeljaison391/Downloads-Organizer#:~:text=4.%20Archives%3A%20,m4a). These will be moved to a **Music** (or **Audio**) folder.
- **Documents:** Office and text files like .pdf, .doc/.docx, .xls/.xlsx, .ppt/.pptx, .odt, .txt, .csv[\[5\]](https://github.com/Joeljaison391/Downloads-Organizer#:~:text=,not%20matching%20the%20above%20categories). These go into a **Documents** folder.
- **Archives:** Compressed archives or disk images (e.g. .zip, .tar, .gz, .7z, .rar, .iso) may be sorted into an **Archives** category (or left in “Others” if not explicitly needed). This is optional but included by default given how common archive files are[\[6\]](https://github.com/Joeljaison391/Downloads-Organizer#:~:text=,not%20matching%20the%20above%20categories).
- **Code Projects:** Any project directory containing source code or version control metadata. A directory that **contains a .git subdirectory** (indicating a Git repository) will be identified as a **Code Project**. Rather than extracting individual files, the entire project folder will be moved intact to a **Projects** directory. This preserves the project structure (including any code, assets, and the .git history) instead of scattering its files. The presence of other version control markers (e.g. .hg for Mercurial, .svn) or a high concentration of source file extensions (.c, .py, .java, etc.) can also be used to identify project directories.
- **Bare Git Repositories:** Occasionally, you might have bare git repo directories (often named with a .git suffix) which contain only the Git history without a working copy. We detect these by directory name or by finding a config file inside with bare = true. Such directories will be moved to a **GitRepos** (or similar) folder. This keeps standalone VCS repositories separate from normal projects.
- **Backups / Home Folders:** If the scan finds directories that appear to be **home directory backups**, those will be consolidated under a **Backups** folder. We detect these by common home-folder patterns: for example, a directory containing typical user subfolders like “Documents” (or localized names like “Documenten” in Dutch), “Downloads”, “Pictures/Afbeeldingen”, “Music/Muziek”, along with dotfiles such as .bash_history, .config, etc. Such a structure strongly suggests a copied home directory. We will move the entire directory under **Backups** (preserving its name, which might encode the username or backup date). This way, complete backups remain intact but out of the way. Multiple home backups (e.g., from different dates or users) will all reside in the Backups category for easier management.
- **Others:** Anything not matched by the above categories will go into an **Others** (or **Misc**) folder. This catches miscellaneous files or directories that don’t fit known types. The user can review this folder manually later.

Using these categories, the tool essentially replicates the concept of “organize files by type into folders of music, PDFs, images, etc.” as seen in existing organizers[\[7\]](https://github.com/toolleeo/awesome-cli-apps-in-a-csv#:~:text=%2A%20classifier%20,same%20extension%20into%20a%20folder). The default behavior sorts each file or folder into one of these top-level categories under the chosen root directory.

## Handling Nested Directories and Metadata Preservation

The tool will traverse all subdirectories of the target path (e.g. /mnt or current directory) recursively to classify content. **Important:** Certain directories should be moved as a whole rather than flattening their contents: for example, identified Code Projects, Git repos, and Backups (as discussed above). When the scanner identifies a directory as one of those categories, it will **skip moving individual files out of it** and instead move the entire directory to the destination category. This preserves internal structure and any metadata encoded in filenames or paths. For instance, if a photo is stored in a dated folder (metadata in path), that context stays if the whole folder moves under “Media” (though in most cases we expect photos to be sorted individually, we won’t disrupt known structured sets like a photo album directory unless explicitly desired). By default, media and documents scattered in arbitrary folders will be pulled out to their categories, but media found inside a recognized project or backup folder will remain with its parent folder to avoid breaking that context.

**Filename/Path Metadata:** The user noted that _filenames or paths may contain metadata they want to keep_. Our approach preserves file names and as much of the original path context as possible. When moving individual files, we keep the original filename. If name collisions occur in the target category (e.g., two different file001.jpg from different folders both going to Media), the tool will rename or prefix one of them to avoid overwriting (for example, add a short identifier from the original path or a numeric suffix). We also record the original path in the log for each moved file. This way, even after files are relocated, the log serves as a reference for where each file came from. In cases where entire directories are moved (projects, backups), the directory name itself is preserved, so any metadata in those names (like dates or source names) remains intact. Optionally, the tool could retain one level of original parent folder structure in the target if that’s useful (perhaps configurable), but by default we’ll aim for a clean, flat category structure for simplicity.

## Removing Unwanted Files (Temp, Cache, Broken)

To free up space, the tool will identify and remove certain files that are clearly unwanted in an archive (this will be an **optional** feature that’s enabled by default but can be turned off for safety). We specifically target:

- **Broken/Invalid Files:** The primary example is _broken symlinks_. These are symbolic links pointing to files that no longer exist. They serve no purpose and can be safely deleted. Tools like rmlint categorize broken links as “lint” (space waste) alongside duplicates[\[8\]](https://rmlint.readthedocs.io/en/v2.4.0/rmlint.1.html#:~:text=,broken%20things%20on%20your%20filesystem). Our tool will scan for symlinks and verify their targets; any link with a missing target is logged and removed. We will also log if we find other corrupt files (for example, if a file’s metadata indicates an error reading it, though such cases are rare and might require user attention rather than auto-delete).
- **Known Temporary or Cache Files:** Many applications create cache directories or temp files that need not be preserved (they can be regenerated by the app). For example:
- Browser caches or thumbnail caches (often in directories named .cache, Cache, or Thumbnails).
- System temp files (files in tmp or Temp directories, or names ending with extensions like .tmp).
- Software-specific caches (e.g. node_modules in project folders can be huge, though these are more dependencies than cache; we won’t remove those by default, but we **will** remove things like thumbs.db or desktop.ini from Windows backups, which are unneeded).
- OS-generated files like .DS_Store (on macOS) or Thumbs.db (Windows Explorer thumbnails) will be deleted on sight.

We will maintain a **default ignore/delete list** of patterns (common cache folders and temp file names). This can include: \*.tmp, \*.part (incomplete download), .~lock\* (temporary lock files), Thumbs.db, desktop.ini, any folder named Cache, .cache, Temporary Internet Files, etc. The user can customize this list in the config. By removing these, we reduce clutter and save space without losing any personal data.

- **Empty Files and Dirs:** As a final cleanup, the tool can remove truly empty files (size 0) and empty directories left behind after moving files. Empty directories might occur once we've moved all their contents. We will log their removal. This ensures no hollow folder structure is left. (This behavior is again optional/configurable, since in some backup scenarios one might want to keep the directory tree; but by default it’s reasonable to trim empties.)

All deletions will be logged in detail and only executed outside of dry-run mode. If the user prefers, they can run in dry mode first to see which files would be removed, then run the actual command. The goal is to only remove files that are safely reproducible or clearly unused, minimizing risk.

## Handling Duplicate Files

Duplicate files are a major cause of wasted space in backups and large data hoards. Because we will be moving files into centralized category folders, duplicates that were once in separate folders may now end up side by side in the same directory, making them easier to spot. Our tool will have an **optional de-duplication pass**: after organizing, it can detect identical files and either **remove or consolidate** them.

For example, the tool could compute hashes (e.g. SHA-1 or a faster non-cryptographic hash) of files to identify duplicates. There are efficient Rust implementations (like the fclones library) that can find groups of identical files and even replace copies with hard links or remove them[\[9\]](https://github.com/pkolaczk/fclones#:~:text=,the%20search%20and%20cleanup%20process)[\[10\]](https://github.com/pkolaczk/fclones#:~:text=,I%2FO%20and%20CPU%20heavy%20stages). By default, we might simply log duplicates found (so the user can decide what to do), or automatically keep one copy in the category folder and either delete others or replace them with a lightweight reference. On Linux, if the filesystem supports reflinks or hardlinks, we could replace duplicates with hardlinks to a single data copy to save space (this could be triggered with a `--deduplicate` flag).

It's important to be safe here: if duplicates are found, the tool will **never delete all copies**; at least one original will be kept. If auto-dedup is on, we would choose one file as the “primary” (perhaps the first encountered or the one in the largest directory) and remove or link the rest. The user will see in the log exactly which duplicates were merged. And just like other operations, a dry run would list duplicate sets and what action would be taken.

By integrating duplicate detection, the tool addresses the “double data” issue (multiple copies of the same file) that often fills disks. This is similar to what dedicated tools like _fclones_ or _rmlint_ do – _fclones,_ for example, groups identical files and can either remove or link them, with a robust preview mode for safety[\[11\]](https://github.com/pkolaczk/fclones#:~:text=,the%20search%20and%20cleanup%20process). Our tool can incorporate a simplified version of this logic or call out to such libraries under the hood.

## Implementation Plan in Rust

We will implement this tool in Rust for performance and safety (Rust’s memory safety will help handle large file operations without crashes). Key aspects of the implementation include:

- **Recursive Directory Traversal:** Using a crate like walkdir or Rust’s fs::read_dir with recursion to iterate through all files and subfolders in the target directory. We will likely first **collect a list of items** to process (to separate scanning from moving). Each item can be categorized then queued for moving. This two-phase approach (scan then move) prevents us from accidentally moving a directory while still scanning inside it. We will also take care to avoid infinite recursion (e.g., not following symlinks by default, since those could point back into the tree or elsewhere). By default, like rmlint, we ignore hidden files (dotfiles) unless they specifically match our criteria (though dotfiles in a home backup are considered for that backup detection, we might not individually move every dotfile except as part of the whole backup folder).
- **File Type Detection:** We will maintain internal maps of extension->category (from our defaults or updated via config). Additionally, we can use the mime_guess or tree_magic crate to get MIME types. If, say, a file has no extension or an unknown one, we can read a small portion of the file and use content-based detection. (For example, an image file with no extension can still be detected by its JPEG/PNG header.) If performance is a concern, we might do content checks only for files above a certain size or those without known extensions. The user can also disable content sniffing if they trust extensions.
- **Moving Files:** We will use std::fs::rename for moving files/directories. This should be an atomic rename on the same filesystem (which is likely the case if everything is under /mnt or one mount point). Renaming (moving) is preferred because it doesn’t require extra disk space. We will create the target category directories at the start (e.g., create Media, Documents, etc under the target root if they don’t exist). Then each file’s move is attempted. Any errors (like permission issues or name collisions) will be caught and logged. Name collisions will be handled by renaming (e.g., appending a number or origin name). We’ll ensure that we **don’t move a directory into one of its own subfolders** (which could happen if the user sets the root to somewhere inside a directory we try to move). Some safety logic: if a directory is identified as e.g. a project, and our target Projects folder is within that same directory, we need to avoid a scenario of moving parent into child. The simplest way: create all category folders at the same level as the items we’re moving (e.g., under /mnt if that’s root), so none of the source items are ancestors of the destination categories.
- **Logging:** Every action will produce a log line. For simplicity, logging to stdout/stderr is fine, but we might also offer a `--log-file` option. At minimum, during dry-run the console output is the log of planned actions. We log lines like “\[DRY-RUN\] MOVE /path/old/file.jpg -> /mnt/Media/file.jpg”. For actual run, similar lines without “DRY-RUN” prefix, and additional lines for deletions (“DELETE /mnt/old/Thumbs.db (Windows thumbnail cache)”). We will also summarize at the end (e.g., “Moved X files, Y directories into categories; removed Z duplicate files; deleted N temp files”). The log will help the user verify that metadata in paths is recorded (original locations are noted).
- **Parallelism:** For potentially faster operation on big datasets, we can use threads (via Rayon) to hash files for duplicates or to move independent files in parallel. Care must be taken not to overwhelm the I/O (maybe limit threads for HDD vs SSD). But by default, a single-threaded approach for moving (which is mostly I/O bound) might be acceptable. We can parallelize the duplicate hashing since that can be CPU-heavy. This is a tunable aspect, and we may add an option like `--threads N`.
- **Robustness:** The tool should handle edge cases: filenames with special characters (spaces, newlines – we will use proper OsStr handling and not assume valid UTF-8 names in all cases), very long paths, permission issues (skip items we cannot read, with a warning), etc. It will _not_ follow symlinks unless an option is given, to avoid messing outside the target tree. If the user wants to include symlink targets, they should run the tool on those target directories separately.

## Configurability and Defaults

We aim for sane defaults that “just work,” but allow customization. The configuration will be loaded from a config directory (for example, $XDG_CONFIG_HOME/dataorganizer/ or ~/.config/dataorganizer/). On first run, if config files are not found, the tool will **create default config files** there with documented default settings. This could include:

- **Category Extension Lists:** e.g., a file media_extensions.txt listing default extensions for Media (jpg, png, mp4, etc), audio_extensions.txt for Music, document_extensions.txt, archive_extensions.txt, etc. The user can edit these to add new file types or remove ones. The tool will read them at start. By default, these contain the common extensions we listed above (which cover most cases)[\[3\]](https://github.com/Joeljaison391/Downloads-Organizer#:~:text=1.%20Images%3A%20,5.%20Audio).
- **Ignore/Remove Lists:** a config file for “ignore_patterns” or “delete_patterns” that lists file name patterns or specific directories to purge (like \*.cache, node_modules/, \*.tmp). We will populate it with known safe defaults (caches, temp files, system junk). The user can tweak it as needed.
- **Options Config:** Possibly a main config file (TOML or JSON format) that sets default behavior flags (e.g., deduplicate = true or remove_empty_dirs = false if they want to change defaults). However, these can also be command-line flags which might be simpler for the user to combine (as requested, “choose and combine methods”). We plan to expose command-line options for each major feature:
- `--dry-run` (no changes, just log actions)
- `--no-dedup` or `--deduplicate` (toggle duplicate file handling)
- `--no-clean` (skip deletion of caches/temp files)
- `--preserve-project-structure` (default true; if false, even files inside projects might be individually categorized – not recommended generally)
- `--target-root <path>` (if user wants to specify a different root to place organized folders, otherwise it uses the given directory itself)
- etc.

By making features optional, the user can, for example, run the tool to only deduplicate a directory without re-organizing (if we allow combining methods arbitrarily). Or they might only want to clean temp files in a run. We could implement subcommands or just flags controlling which actions to execute.

Linux also has built-in file type databases (like mime types mapping) and the file tool[\[1\]](https://www.hostinger.com/tutorials/linux-file-command#:~:text=When%20executed%2C%20the%20command%20doesn%E2%80%99t,to%20determine%20the%20file%20type); our default extension lists plus libmagic use should cover most needs without requiring the user to supply their own lists. However, if needed, advanced users could point the tool to an alternate mime definition file or extend the categories by adding new config files (the tool could treat any file in the config dir named \*.txt as a category definition: e.g., user could create videos_extensions.txt to split videos out of Media if they wanted, and the tool could support that dynamically).

## Logging and Dry-Run Mode

Logging is critical for user confidence, so every action the tool takes (or plans to take) will be printed. **Dry-run mode** (-n or `--dry-run`) will go through the entire process without actually moving or deleting anything, and output all the intended actions. This lets the user review what would happen. For example, a dry run might output:

```sh
[DRY RUN] Would move "./old_photos/vacation.jpg" -> "Media/vacation.jpg"  
[DRY RUN] Would move "./Downloads/song.mp3" -> "Music/song.mp3"  
[DRY RUN] Would move "./backup/home-john/Documents/report.docx" -> "Backups/home-john/Documents/report.docx"  
[DRY RUN] Would remove "./backup/home-john/.cache/" (cache directory)  
[DRY RUN] Would remove duplicate "Documents/report (1).docx", identical to "Documents/report.docx"
```

Each line identifies the source and planned destination or removal. After reviewing, the user can run without dry-run to perform the changes. During actual execution, similar lines will be logged (without the "\[DRY RUN\]" prefix). We’ll ensure the log is easy to read and possibly color-code or label actions (e.g. "MOVE", "DELETE", "SKIP") for clarity.

It’s worth noting that even **fclones** (the Rust duplicate finder) emphasizes dry-run for safety[\[12\]](https://github.com/pkolaczk/fclones#:~:text=,file%20system%20would%20be%20made), and we follow that principle. The user should feel in control and able to abort or adjust before any irreversible changes happen.

## Example Usage

Imagine the user runs the tool in /mnt which contains a messy collection. By default, the tool will create an organized structure under /mnt itself, like /mnt/Media, /mnt/Music, /mnt/Documents, /mnt/Projects, /mnt/GitRepos, /mnt/Backups, /mnt/Others. It will then move files accordingly.

- All images and videos anywhere under /mnt (except those inside identified project or backup folders) end up under /mnt/Media. For example, /mnt/old_photos/pic1.png -> /mnt/Media/pic1.png. If there were subfolders of photos, it might recreate those subfolders inside Media (optionally), or flatten them – we can decide default to flatten, but log the original path.
- A folder /mnt/my_old_code/ that contains a .git directory will be moved entirely to /mnt/Projects/my_old_code/.
- A bare repo like /mnt/backup_project.git/ (just a Git folder) will move to /mnt/GitRepos/backup_project.git/.
- A directory /mnt/home-john-backup/ containing Desktop, Documents, Pictures, .bashrc... is detected as a home backup and moved to /mnt/Backups/home-john-backup/ (retaining all its internal structure).
- All .mp3 and audio files scattered around go to /mnt/Music/. Similarly, documents to /mnt/Documents/.
- Anything unrecognized (say an .apk file or unknown extension) goes to /mnt/Others/.

After moving, the tool scans each category for duplicates. Suppose song.mp3 was found in two places originally, now both copies reside in /mnt/Music; the tool will detect they are identical and might remove one, logging which one was removed as duplicate. It will also purge things like /mnt/Backups/home-john-backup/.cache/ entirely, and delete stray Thumbs.db files from Windows backup folders.

Finally, it prints a summary: perhaps “Moved 500 files into 7 categories. Consolidated 50 duplicate files saving 200MB. Removed 300MB of cache/temp files. Dry run recommended for review of actions.” (The summary phrasing can be refined in implementation.)

## Conclusion

This Rust console tool will dramatically simplify organizing a bloated data directory by **classifying files into logical categories** and **eliminating redundant data**. By using both file name extensions and content-based detection, it can intelligently sort photos, videos, music, documents, code repositories, and more into appropriate folders (much like existing file organizers)[\[7\]](https://github.com/toolleeo/awesome-cli-apps-in-a-csv#:~:text=%2A%20classifier%20,same%20extension%20into%20a%20folder). It leverages Linux file type info (e.g., the file utility’s approach) to recognize files even if extensions are missing[\[1\]](https://www.hostinger.com/tutorials/linux-file-command#:~:text=When%20executed%2C%20the%20command%20doesn%E2%80%99t,to%20determine%20the%20file%20type), and employs robust strategies to detect special cases like home directory backups and Git repositories. Unnecessary files (caches, broken links, duplicates) are cleaned up to free space[\[8\]](https://rmlint.readthedocs.io/en/v2.4.0/rmlint.1.html#:~:text=,broken%20things%20on%20your%20filesystem)[\[9\]](https://github.com/pkolaczk/fclones#:~:text=,the%20search%20and%20cleanup%20process). All operations are transparent with thorough logging and a no-action dry-run mode for safety[\[12\]](https://github.com/pkolaczk/fclones#:~:text=,file%20system%20would%20be%20made).

By making the behavior highly configurable (with default configs that cover most common cases), the user can tailor the tool to their needs—choosing which categories to sort by, which files to purge, and whether to perform deduplication. Once implemented, this tool will turn a chaotic mass of files into a structured repository, making it far easier to manage and find data, all while reclaiming disk space lost to duplicates and junk files.

[\[1\]](https://www.hostinger.com/tutorials/linux-file-command#:~:text=When%20executed%2C%20the%20command%20doesn%E2%80%99t,to%20determine%20the%20file%20type) What Is Linux File Command and How To Determine File Type

<https://www.hostinger.com/tutorials/linux-file-command>

[\[2\]](https://stackoverflow.com/questions/1735659/list-of-all-mimetypes-on-the-planet-mapped-to-file-extensions#:~:text=9) mime types - List of ALL MimeTypes on the Planet, mapped to File Extensions? - Stack Overflow

<https://stackoverflow.com/questions/1735659/list-of-all-mimetypes-on-the-planet-mapped-to-file-extensions>

[\[3\]](https://github.com/Joeljaison391/Downloads-Organizer#:~:text=1.%20Images%3A%20,5.%20Audio) [\[4\]](https://github.com/Joeljaison391/Downloads-Organizer#:~:text=4.%20Archives%3A%20,m4a) [\[5\]](https://github.com/Joeljaison391/Downloads-Organizer#:~:text=,not%20matching%20the%20above%20categories) [\[6\]](https://github.com/Joeljaison391/Downloads-Organizer#:~:text=,not%20matching%20the%20above%20categories) GitHub - Joeljaison391/Downloads-Organizer: A Rust application designed to automate the organization of your downloads. It watches for new files and moves them to appropriate directories based on their extension, simplifying file management and improving workflow.

<https://github.com/Joeljaison391/Downloads-Organizer>

[\[7\]](https://github.com/toolleeo/awesome-cli-apps-in-a-csv#:~:text=%2A%20classifier%20,same%20extension%20into%20a%20folder) GitHub - toolleeo/awesome-cli-apps-in-a-csv: The largest Awesome Curated list of command line programs (CLI/TUI) with source data organized into CSV files

<https://github.com/toolleeo/awesome-cli-apps-in-a-csv>

[\[8\]](https://rmlint.readthedocs.io/en/v2.4.0/rmlint.1.html#:~:text=,broken%20things%20on%20your%20filesystem) rmlint — rmlint (2.4.0 Myopic Micrathene) documentation

<https://rmlint.readthedocs.io/en/v2.4.0/rmlint.1.html>

[\[9\]](https://github.com/pkolaczk/fclones#:~:text=,the%20search%20and%20cleanup%20process) [\[10\]](https://github.com/pkolaczk/fclones#:~:text=,I%2FO%20and%20CPU%20heavy%20stages) [\[11\]](https://github.com/pkolaczk/fclones#:~:text=,the%20search%20and%20cleanup%20process) [\[12\]](https://github.com/pkolaczk/fclones#:~:text=,file%20system%20would%20be%20made) GitHub - pkolaczk/fclones: Efficient Duplicate File Finder

<https://github.com/pkolaczk/fclones>
