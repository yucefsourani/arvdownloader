use gtk::glib;
use std::path::{Path, PathBuf};

pub const APP_ID: &str = "com.github.yucefsourani.arvdownloader";
pub const APP_NAME: &str = "arvdownloader";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_DEVELOPER: &str = "Yucef Sourani";
pub const APP_WEBSITE: &str = "https://github.com/yucefsourani/arvdownloader";
pub const APP_ISSUE_URL: &str = "https://github.com/yucefsourani/arvdownloader/issues";
pub const ICON_NAME: &str = "com.github.yucefsourani.arvdownloader";
pub const ICON_FILE_PNG: &str = "data/icons/com.github.yucefsourani.arvdownloader.png";
pub const ICON_FILE_ICO: &str = "data/icons/icon.ico";

/// Hidden folder created inside the user's Home Directory on Linux (`~/.arvdownloader`)
pub const LINUX_DATA_DIR_NAME: &str = ".arvdownloader";

/// Folder created inside `%LOCALAPPDATA%` on Windows (`%LOCALAPPDATA%\arvdownloader`)
pub const WINDOWS_DATA_DIR_NAME: &str = "arvdownloader";

/// Legacy folder name from older versions used for automatic one-time migration
pub const LEGACY_DATA_DIR_NAME: &str = ".video_downloader";

pub const YTDLP_GITHUB_API: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
pub const FFMPEG_GITHUB_API: &str =
    "https://api.github.com/repos/yt-dlp/FFmpeg-Builds/releases/latest";
pub const DENO_GITHUB_API: &str = "https://api.github.com/repos/denoland/deno/releases/latest";

pub const YTDLP_DIRECT_LINUX_URL: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux";
pub const YTDLP_DIRECT_WINDOWS_URL: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
pub const FFMPEG_DIRECT_LINUX_URL: &str =
    "https://github.com/yt-dlp/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-linux64-gpl.tar.xz";
pub const FFMPEG_DIRECT_WINDOWS_URL: &str =
    "https://github.com/yt-dlp/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-win64-gpl.zip";
pub const DENO_DIRECT_LINUX_URL: &str =
    "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-unknown-linux-gnu.zip";
pub const DENO_DIRECT_WINDOWS_URL: &str =
    "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip";

pub const HTTP_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:132.0) Gecko/20100101 Firefox/132.0 arvdownloader/0.1.0";

/// Returns the platform-appropriate application data directory:
/// - **Linux**: `~/.arvdownloader` (hidden folder in user's Home directory)
/// - **Windows**: `%LOCALAPPDATA%\arvdownloader` (inside hidden `AppData\Local`, avoiding visible folders in `%USERPROFILE%`)
pub fn app_home_dir() -> PathBuf {
    #[cfg(windows)]
    {
        let base = std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("APPDATA"))
            .map(PathBuf::from)
            .unwrap_or_else(|| glib::user_data_dir());
        let dir = base.join(WINDOWS_DATA_DIR_NAME);
        let _ = std::fs::create_dir_all(&dir);
        return normalize_path(&dir);
    }
    #[cfg(not(windows))]
    {
        let home = glib::home_dir();
        let dir = home.join(LINUX_DATA_DIR_NAME);
        let _ = std::fs::create_dir_all(&dir);
        normalize_path(&dir)
    }
}

/// Returns the legacy `~/.video_downloader` path if it exists on disk (for one-time migration)
pub fn legacy_app_home_dir() -> Option<PathBuf> {
    let legacy = glib::home_dir().join(LEGACY_DATA_DIR_NAME);
    if legacy.exists() {
        Some(normalize_path(&legacy))
    } else {
        None
    }
}

/// Human-friendly display label for the managed binary directory
pub fn managed_bin_dir_display() -> &'static str {
    if cfg!(windows) {
        r"%LOCALAPPDATA%\arvdownloader\bin"
    } else {
        "~/.arvdownloader/bin"
    }
}

/// Returns the managed binary directory: `~/.arvdownloader/bin` (Linux) or `%LOCALAPPDATA%\arvdownloader\bin` (Windows)
pub fn managed_bin_dir() -> PathBuf {
    let bin = app_home_dir().join("bin");
    let _ = std::fs::create_dir_all(&bin);
    normalize_path(&bin)
}

/// Returns the staging directory used during downloads and archive extraction
pub fn staging_dir() -> PathBuf {
    let staging = app_home_dir().join("staging");
    let _ = std::fs::create_dir_all(&staging);
    normalize_path(&staging)
}

/// Returns the path to `versions.json` inside `managed_bin_dir()`
pub fn versions_file_path() -> PathBuf {
    managed_bin_dir().join("versions.json")
}

/// Returns the path to `config.json` inside `app_home_dir()`
pub fn config_file_path() -> PathBuf {
    app_home_dir().join("config.json")
}

/// Returns the path to `history.json` inside `app_home_dir()`
pub fn history_file_path() -> PathBuf {
    app_home_dir().join("history.json")
}

/// Platform-aware binary filename for `yt-dlp`
pub fn ytdlp_bin_name() -> &'static str {
    if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    }
}

/// Platform-aware binary filename for `ffmpeg`
pub fn ffmpeg_bin_name() -> &'static str {
    if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    }
}

/// Platform-aware binary filename for `ffprobe`
pub fn ffprobe_bin_name() -> &'static str {
    if cfg!(windows) {
        "ffprobe.exe"
    } else {
        "ffprobe"
    }
}

/// Platform-aware binary filename for `deno` (JS runtime for YouTube EJS signature solving)
pub fn deno_bin_name() -> &'static str {
    if cfg!(windows) {
        "deno.exe"
    } else {
        "deno"
    }
}

/// Managed `yt-dlp` executable path in `managed_bin_dir()`
pub fn managed_ytdlp_path() -> PathBuf {
    managed_bin_dir().join(ytdlp_bin_name())
}

/// Managed `ffmpeg` executable path in `managed_bin_dir()`
pub fn managed_ffmpeg_path() -> PathBuf {
    managed_bin_dir().join(ffmpeg_bin_name())
}

/// Managed `ffprobe` executable path in `managed_bin_dir()`
pub fn managed_ffprobe_path() -> PathBuf {
    managed_bin_dir().join(ffprobe_bin_name())
}

/// Managed `deno` executable path in `managed_bin_dir()`
pub fn managed_deno_path() -> PathBuf {
    managed_bin_dir().join(deno_bin_name())
}

/// Returns all existing candidate directories where pre-bundled or legacy binaries may reside
/// (`<exe_dir>/bundled_bin`, `<exe_dir>/bin`, `<exe_dir>`, and legacy `~/.video_downloader/bin`).
pub fn candidate_bundled_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            for sub in ["bundled_bin", "bin"] {
                let candidate = parent.join(sub);
                if candidate.exists() && candidate.is_dir() {
                    dirs.push(normalize_path(&candidate));
                }
            }
            if parent.exists() && parent.is_dir() {
                dirs.push(normalize_path(parent));
            }
        }
    }
    if let Some(legacy_home) = legacy_app_home_dir() {
        let legacy_bin = legacy_home.join("bin");
        if legacy_bin.exists() && legacy_bin.is_dir() {
            dirs.push(normalize_path(&legacy_bin));
        }
    }
    dirs
}

/// Default output folder for downloaded videos/audio
pub fn default_download_dir() -> PathBuf {
    if let Some(dl) = glib::user_special_dir(glib::UserDirectory::Downloads) {
        let dir = dl.join("arvdownloader");
        let _ = std::fs::create_dir_all(&dir);
        return normalize_path(&dir);
    }
    let fallback = glib::home_dir().join("Downloads").join("arvdownloader");
    let _ = std::fs::create_dir_all(&fallback);
    normalize_path(&fallback)
}

/// Normalizes paths safely across Windows (removing `\\?\` UNC prefixes via `dunce`) and Linux
pub fn normalize_path(path: &Path) -> PathBuf {
    dunce::simplified(path).to_path_buf()
}

/// Strips non-BMP color emoji ranges on Windows to prevent DirectWrite/Pango/Vulkan crashes in `AdwToast`
pub fn sanitize_toast_text(input: &str) -> String {
    input
        .chars()
        .filter(|&c| !('\u{1F300}'..='\u{1FAFF}').contains(&c))
        .collect()
}

/// Checks if a base name matches a Windows reserved DOS device name (`CON`, `PRN`, `AUX`, `NUL`, `COM1`..`COM9`, `LPT1`..`LPT9`).
pub fn is_windows_reserved_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).trim();
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

/// Sanitizes a file or folder name component so that it is 100% valid and safe across both
/// **Linux** (`NAME_MAX` = 255 UTF-8 bytes) and **Windows** (`MAX_PATH` = 260 UTF-16 chars, no `<>:"/\|?*`,
/// no control/bidi characters, no trailing spaces or periods, no reserved DOS names).
pub fn sanitize_fs_name(input: &str, max_chars: usize, max_utf8_bytes: usize) -> String {
    let mut cleaned = String::with_capacity(input.len());

    for c in input.chars() {
        // Strip ASCII control characters and invisible Unicode zero-width / bidirectional formatting characters
        if c.is_control()
            || ('\u{200B}'..='\u{200F}').contains(&c)
            || ('\u{202A}'..='\u{202E}').contains(&c)
            || ('\u{2060}'..='\u{206F}').contains(&c)
            || c == '\u{FEFF}'
        {
            continue;
        }

        // Replace filesystem-forbidden characters on Windows and Linux with safe separators
        match c {
            ':' | '|' => {
                cleaned.push_str(" - ");
            }
            '<' | '>' | '"' | '/' | '\\' | '?' | '*' => {
                cleaned.push(' ');
            }
            _ if c.is_whitespace() => {
                cleaned.push(' ');
            }
            _ => {
                cleaned.push(c);
            }
        }
    }

    // Collapse consecutive spaces into a single space
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");

    // Trim leading/trailing dots, spaces, and hyphens
    let trimmed = collapsed.trim_matches(|c: char| c == ' ' || c == '.');

    // Enforce both character limit (for Windows MAX_PATH) and UTF-8 byte limit (for Linux 255-byte NAME_MAX)
    // strictly on valid Unicode scalar value boundaries.
    let mut result = String::new();
    let mut char_count = 0usize;
    let mut byte_count = 0usize;

    for c in trimmed.chars() {
        let c_bytes = c.len_utf8();
        if char_count + 1 > max_chars || byte_count + c_bytes > max_utf8_bytes {
            break;
        }
        result.push(c);
        char_count += 1;
        byte_count += c_bytes;
    }

    // Re-trim trailing spaces, periods, and hyphens after truncation (critical on Windows!)
    let final_str = result
        .trim_end_matches(|c: char| c == ' ' || c == '.' || c == '-')
        .trim_start_matches(|c: char| c == ' ' || c == '.')
        .to_string();

    if final_str.is_empty() {
        return "Playlist".to_string();
    }

    if is_windows_reserved_device_name(&final_str) {
        format!("_{}", final_str)
    } else {
        final_str
    }
}

/// Computes safe `(folder_max_chars, title_max_chars)` budgets for `yt-dlp` output templates and folders.
///
/// Guarantees:
/// 1. On **Windows**: Full path `<base_dir>\<playlist_folder>\<01 - title [id].ext.f137.part>` stays well under
///    240 characters (strictly below `MAX_PATH = 260`), even if `base_dir` is deeply nested.
/// 2. On **Linux**: Each directory and filename component stays well under 200 UTF-8 bytes
///    (strictly below `NAME_MAX = 255` bytes), even if the title is 100% Arabic (2 bytes/char),
///    CJK/fullwidth Unicode (3 bytes/char), or Emoji (4 bytes/char).
pub fn compute_safe_path_limits(base_dir: &Path, is_playlist: bool) -> (usize, usize) {
    let base_chars = base_dir.to_string_lossy().chars().count();
    // Reserve ~38 chars for separators + `001 - ` + ` [11-char-id].mp4.f137.part`
    let reserved_suffix_chars = 38usize;
    // Target maximum full path of 238 chars to stay safely below Windows 260-char MAX_PATH
    let available_budget = 238usize.saturating_sub(base_chars + reserved_suffix_chars);

    if is_playlist {
        // Split budget between playlist subfolder name and video title
        // Cap folder at 48 chars (max 144 bytes in 3-byte UTF-8, 96 bytes in Arabic)
        // Cap video title at 52 chars (max 156 bytes in 3-byte UTF-8, 104 bytes in Arabic)
        let folder_max = (available_budget * 45 / 100).clamp(18, 48);
        let title_max = (available_budget.saturating_sub(folder_max + 1)).clamp(20, 52);
        (folder_max, title_max)
    } else {
        let title_max = available_budget.clamp(25, 68);
        (0, title_max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_fs_name_arabic_and_forbidden_chars() {
        let raw = "  دورة تعلم Rust: من الصفر | الحلقة 1? <2026>..  ";
        let safe = sanitize_fs_name(raw, 48, 140);
        assert_eq!(safe, "دورة تعلم Rust - من الصفر - الحلقة 1 2026");
        assert!(!safe.ends_with('.'));
        assert!(!safe.ends_with(' '));
        assert!(safe.len() <= 140);
        assert!(safe.chars().count() <= 48);
    }

    #[test]
    fn test_sanitize_fs_name_windows_reserved_and_length() {
        assert_eq!(sanitize_fs_name("CON", 48, 140), "_CON");
        assert_eq!(sanitize_fs_name("aux.txt", 48, 140), "_aux.txt");

        let long_arabic = "سلسلة دروس برمجة تطبيقات سطح المكتب المتقاطعة باستخدام لغة رست وواجهة جيتيكي الحديثة";
        let truncated = sanitize_fs_name(long_arabic, 35, 100);
        assert!(truncated.chars().count() <= 35);
        assert!(truncated.len() <= 100);
        assert!(!truncated.ends_with(' '));
    }

    #[test]
    fn test_compute_safe_path_limits_windows_and_linux() {
        let deep_win_path = Path::new(r"C:\Users\Administrator\Downloads\arvdownloader\Subfolder");
        let (folder_lim, title_lim) = compute_safe_path_limits(deep_win_path, true);
        let total_estimated =
            deep_win_path.to_string_lossy().chars().count() + 1 + folder_lim + 1 + title_lim + 38;
        assert!(total_estimated <= 245, "Should stay well below 260 MAX_PATH");
        assert!(folder_lim <= 48);
        assert!(title_lim <= 52);
    }
}
