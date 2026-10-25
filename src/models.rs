use crate::metadata;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityPreset {
    BestVideo,
    Video1080p,
    Video720p,
    Video480p,
    AudioMp3,
    AudioM4a,
    CustomFormat {
        format_id: String,
        label: String,
        is_audio_only: bool,
    },
}

impl QualityPreset {
    pub fn all_standard() -> &'static [QualityPreset] {
        &[
            QualityPreset::BestVideo,
            QualityPreset::Video1080p,
            QualityPreset::Video720p,
            QualityPreset::Video480p,
            QualityPreset::AudioMp3,
            QualityPreset::AudioM4a,
        ]
    }

    pub fn display_name(&self) -> String {
        match self {
            QualityPreset::BestVideo => "Best Video (Up to 4K MP4)".to_string(),
            QualityPreset::Video1080p => "1080p Full HD (MP4)".to_string(),
            QualityPreset::Video720p => "720p HD (MP4)".to_string(),
            QualityPreset::Video480p => "480p Standard (MP4)".to_string(),
            QualityPreset::AudioMp3 => "Audio Only (MP3 Best)".to_string(),
            QualityPreset::AudioM4a => "Audio Only (M4A)".to_string(),
            QualityPreset::CustomFormat { label, .. } => label.clone(),
        }
    }

    pub fn short_badge(&self) -> String {
        match self {
            QualityPreset::BestVideo => "Best MP4".to_string(),
            QualityPreset::Video1080p => "1080p MP4".to_string(),
            QualityPreset::Video720p => "720p MP4".to_string(),
            QualityPreset::Video480p => "480p MP4".to_string(),
            QualityPreset::AudioMp3 => "MP3 Audio".to_string(),
            QualityPreset::AudioM4a => "M4A Audio".to_string(),
            QualityPreset::CustomFormat { label, .. } => label.clone(),
        }
    }

    pub fn from_index(idx: u32) -> Self {
        match idx {
            0 => QualityPreset::BestVideo,
            1 => QualityPreset::Video1080p,
            2 => QualityPreset::Video720p,
            3 => QualityPreset::Video480p,
            4 => QualityPreset::AudioMp3,
            5 => QualityPreset::AudioM4a,
            _ => QualityPreset::BestVideo,
        }
    }

    pub fn to_index(&self) -> u32 {
        match self {
            QualityPreset::BestVideo => 0,
            QualityPreset::Video1080p => 1,
            QualityPreset::Video720p => 2,
            QualityPreset::Video480p => 3,
            QualityPreset::AudioMp3 => 4,
            QualityPreset::AudioM4a => 5,
            QualityPreset::CustomFormat { .. } => 0,
        }
    }

    pub fn is_audio(&self) -> bool {
        matches!(
            self,
            QualityPreset::AudioMp3
                | QualityPreset::AudioM4a
                | QualityPreset::CustomFormat {
                    is_audio_only: true,
                    ..
                }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserCookieSource {
    None,
    Firefox,
    Chrome,
    Brave,
    Edge,
    Chromium,
    Vivaldi,
    Opera,
    CustomFile,
}

impl BrowserCookieSource {
    pub fn all() -> &'static [BrowserCookieSource] {
        &[
            BrowserCookieSource::None,
            BrowserCookieSource::Firefox,
            BrowserCookieSource::Chrome,
            BrowserCookieSource::Brave,
            BrowserCookieSource::Edge,
            BrowserCookieSource::Chromium,
            BrowserCookieSource::Vivaldi,
            BrowserCookieSource::Opera,
            BrowserCookieSource::CustomFile,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            BrowserCookieSource::None => "No Cookies (Public Mode)",
            BrowserCookieSource::Firefox => "Firefox Browser Cookies",
            BrowserCookieSource::Chrome => "Google Chrome Cookies",
            BrowserCookieSource::Brave => "Brave Browser Cookies",
            BrowserCookieSource::Edge => "Microsoft Edge Cookies",
            BrowserCookieSource::Chromium => "Chromium Cookies",
            BrowserCookieSource::Vivaldi => "Vivaldi Cookies",
            BrowserCookieSource::Opera => "Opera Cookies",
            BrowserCookieSource::CustomFile => "Custom cookies.txt File",
        }
    }

    pub fn short_toolbar_label(&self) -> String {
        match self {
            BrowserCookieSource::None => "Cookies: Off".to_string(),
            BrowserCookieSource::CustomFile => "Cookies: File".to_string(),
            _ => {
                let short_name = match self {
                    BrowserCookieSource::Firefox => "Firefox",
                    BrowserCookieSource::Chrome => "Chrome",
                    BrowserCookieSource::Brave => "Brave",
                    BrowserCookieSource::Edge => "Edge",
                    BrowserCookieSource::Chromium => "Chromium",
                    BrowserCookieSource::Vivaldi => "Vivaldi",
                    BrowserCookieSource::Opera => "Opera",
                    _ => "",
                };
                if self.is_available_on_system() {
                    format!("Cookies: {} ✓", short_name)
                } else {
                    format!("Cookies: {} (N/A)", short_name)
                }
            }
        }
    }

    pub fn detailed_settings_label(&self) -> String {
        match self {
            BrowserCookieSource::None | BrowserCookieSource::CustomFile => {
                self.display_name().to_string()
            }
            _ => {
                if self.is_available_on_system() {
                    format!("{} (Detected ✓)", self.display_name())
                } else {
                    format!("{} (Not Found on System)", self.display_name())
                }
            }
        }
    }

    pub fn ytdlp_browser_arg(&self) -> Option<&'static str> {
        match self {
            BrowserCookieSource::None | BrowserCookieSource::CustomFile => None,
            BrowserCookieSource::Firefox => Some("firefox"),
            BrowserCookieSource::Chrome => Some("chrome"),
            BrowserCookieSource::Brave => Some("brave"),
            BrowserCookieSource::Edge => Some("edge"),
            BrowserCookieSource::Chromium => Some("chromium"),
            BrowserCookieSource::Vivaldi => Some("vivaldi"),
            BrowserCookieSource::Opera => Some("opera"),
        }
    }

    /// Checks whether this browser has a valid cookies database on the current OS (Native, Flatpak, Snap, or Windows).
    pub fn is_available_on_system(&self) -> bool {
        match self {
            BrowserCookieSource::None | BrowserCookieSource::CustomFile => true,
            _ => self.resolve_cookie_browser_spec().is_some(),
        }
    }

    /// Resolves the exact `BROWSER` or `BROWSER:PROFILE_PATH` specification for `yt-dlp --cookies-from-browser`.
    /// Returns `None` if the selected browser has no cookies database on the system.
    pub fn resolve_cookie_browser_spec(&self) -> Option<String> {
        let browser_name = self.ytdlp_browser_arg()?;
        let home = gtk::glib::home_dir();

        if *self == BrowserCookieSource::Firefox {
            return resolve_firefox_cookie_spec(&home);
        }

        resolve_chromium_family_cookie_spec(browser_name, *self, &home)
    }

    pub fn from_index(idx: u32) -> Self {
        Self::all()
            .get(idx as usize)
            .copied()
            .unwrap_or(BrowserCookieSource::None)
    }

    pub fn to_index(&self) -> u32 {
        Self::all()
            .iter()
            .position(|b| b == self)
            .unwrap_or(0) as u32
    }
}

fn resolve_firefox_cookie_spec(home: & std::path::Path) -> Option<String> {
    let mut base_dirs = vec![
        home.join(".mozilla/firefox"),
        home.join(".var/app/org.mozilla.firefox/.mozilla/firefox"),
        home.join("snap/firefox/common/.mozilla/firefox"),
        home.join(".config/mozilla/firefox"),
    ];
    if let Ok(appdata) = std::env::var("APPDATA") {
        base_dirs.push(PathBuf::from(appdata).join("Mozilla/Firefox/Profiles"));
    }

    let mut best_profile: Option<(PathBuf, std::time::SystemTime, u64)> = None;

    for base in base_dirs {
        if !base.exists() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let cookie_db = path.join("cookies.sqlite");
            if let Ok(meta) = std::fs::metadata(&cookie_db) {
                if meta.len() > 0 {
                    let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                    let len = meta.len();
                    let should_replace = match &best_profile {
                        None => true,
                        Some((_, best_mod, best_len)) => {
                            (modified, len) > (*best_mod, *best_len)
                        }
                    };
                    if should_replace {
                        best_profile = Some((path, modified, len));
                    }
                }
            }
        }
    }

    best_profile.map(|(profile_dir, _, _)| {
        format!("firefox:{}", metadata::normalize_path(&profile_dir).to_string_lossy())
    })
}

fn resolve_chromium_family_cookie_spec(
    browser_name: &str,
    source: BrowserCookieSource,
    home: &std::path::Path,
) -> Option<String> {
    let mut candidates: Vec<(PathBuf, bool)> = Vec::new(); // (base_dir, is_standard_default_location)
    let local_appdata = std::env::var("LOCALAPPDATA").ok().map(PathBuf::from);
    let roaming_appdata = std::env::var("APPDATA").ok().map(PathBuf::from);

    match source {
        BrowserCookieSource::Chrome => {
            candidates.push((home.join(".config/google-chrome"), true));
            candidates.push((
                home.join(".var/app/com.google.Chrome/config/google-chrome"),
                false,
            ));
            if let Some(ref local) = local_appdata {
                candidates.push((local.join("Google/Chrome/User Data"), true));
            }
        }
        BrowserCookieSource::Brave => {
            candidates.push((
                home.join(".config/BraveSoftware/Brave-Browser"),
                true,
            ));
            candidates.push((
                home.join(".var/app/com.brave.Browser/config/BraveSoftware/Brave-Browser"),
                false,
            ));
            candidates.push((
                home.join("snap/brave/current/.config/BraveSoftware/Brave-Browser"),
                false,
            ));
            if let Some(ref local) = local_appdata {
                candidates.push((
                    local.join("BraveSoftware/Brave-Browser/User Data"),
                    true,
                ));
            }
        }
        BrowserCookieSource::Edge => {
            candidates.push((home.join(".config/microsoft-edge"), true));
            candidates.push((
                home.join(".var/app/com.microsoft.Edge/config/microsoft-edge"),
                false,
            ));
            if let Some(ref local) = local_appdata {
                candidates.push((local.join("Microsoft/Edge/User Data"), true));
            }
        }
        BrowserCookieSource::Chromium => {
            candidates.push((home.join(".config/chromium"), true));
            candidates.push((
                home.join(".var/app/org.chromium.Chromium/config/chromium"),
                false,
            ));
            candidates.push((home.join("snap/chromium/common/chromium"), false));
            if let Some(ref local) = local_appdata {
                candidates.push((local.join("Chromium/User Data"), true));
            }
        }
        BrowserCookieSource::Vivaldi => {
            candidates.push((home.join(".config/vivaldi"), true));
            candidates.push((
                home.join(".var/app/com.vivaldi.Vivaldi/config/vivaldi"),
                false,
            ));
            if let Some(ref local) = local_appdata {
                candidates.push((local.join("Vivaldi/User Data"), true));
            }
        }
        BrowserCookieSource::Opera => {
            candidates.push((home.join(".config/opera"), true));
            candidates.push((
                home.join(".var/app/com.opera.Opera/config/opera"),
                false,
            ));
            candidates.push((home.join("snap/opera/current/.config/opera"), false));
            if let Some(ref roaming) = roaming_appdata {
                candidates.push((roaming.join("Opera Software/Opera Stable"), true));
            }
        }
        _ => return None,
    }

    let sub_cookie_paths = [
        "Default/Cookies",
        "Default/Network/Cookies",
        "Cookies",
        "Network/Cookies",
        "Profile 1/Cookies",
        "Profile 1/Network/Cookies",
        "Profile 2/Cookies",
        "Profile 2/Network/Cookies",
    ];

    for (base_dir, is_standard) in candidates {
        if !base_dir.exists() {
            continue;
        }
        for rel in sub_cookie_paths {
            let cookie_file = base_dir.join(rel);
            if let Ok(meta) = std::fs::metadata(&cookie_file) {
                if meta.len() > 0 {
                    if is_standard && (rel.starts_with("Default/") || rel == "Cookies" || rel == "Network/Cookies") {
                        return Some(browser_name.to_string());
                    }
                    // If in a numbered profile or Flatpak/Snap path, pass exact profile directory
                    let profile_dir = if let Some(prefix) = rel.split('/').next() {
                        if prefix.starts_with("Profile ") || prefix == "Default" {
                            base_dir.join(prefix)
                        } else {
                            base_dir.clone()
                        }
                    } else {
                        base_dir.clone()
                    };
                    return Some(format!(
                        "{}:{}",
                        browser_name,
                        metadata::normalize_path(&profile_dir).to_string_lossy()
                    ));
                }
            }
        }
    }

    None
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub download_dir: PathBuf,
    pub default_preset: QualityPreset,
    pub cookie_source: BrowserCookieSource,
    pub custom_cookies_path: Option<PathBuf>,
    pub proxy_url: String,
    pub embed_thumbnail: bool,
    pub embed_metadata: bool,
    pub sponsorblock_remove: bool,
    pub auto_bypass_fallbacks: bool,
    pub check_updates_on_startup: bool,
    #[serde(default = "default_true")]
    pub use_managed_deno: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            download_dir: metadata::default_download_dir(),
            default_preset: QualityPreset::BestVideo,
            cookie_source: BrowserCookieSource::None,
            custom_cookies_path: None,
            proxy_url: String::new(),
            embed_thumbnail: true,
            embed_metadata: true,
            sponsorblock_remove: false,
            auto_bypass_fallbacks: true,
            check_updates_on_startup: true,
            use_managed_deno: true,
        }
    }
}

impl AppSettings {
    pub fn load() -> Self {
        let path = metadata::config_file_path();
        if let Ok(data) = std::fs::read_to_string(&path) {
            let clean = data.trim_start_matches('\u{feff}');
            if let Ok(mut cfg) = serde_json::from_str::<AppSettings>(clean) {
                let _ = std::fs::create_dir_all(&cfg.download_dir);
                cfg.download_dir = metadata::normalize_path(&cfg.download_dir);
                return cfg;
            }
        }
        let default = Self::default();
        default.save();
        default
    }

    pub fn save(&self) {
        let path = metadata::config_file_path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstalledToolVersions {
    pub ytdlp_version: String,
    pub ffmpeg_version: String,
    #[serde(default)]
    pub deno_version: String,
}

impl InstalledToolVersions {
    pub fn load() -> Self {
        let path = metadata::versions_file_path();
        if let Ok(content) = std::fs::read_to_string(path) {
            let clean = content.trim_start_matches('\u{feff}');
            if let Ok(v) = serde_json::from_str::<InstalledToolVersions>(clean) {
                return v;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = metadata::versions_file_path();
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, content);
        }
    }
}

#[derive(Debug, Clone)]
pub enum ToolUpdateEvent {
    Started {
        message: String,
    },
    Progress {
        tool_name: String,
        fraction: f64,
        status_text: String,
    },
    Completed {
        ytdlp_version: String,
        ffmpeg_version: String,
        deno_version: String,
        updated_any: bool,
    },
    Warning {
        message: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatOption {
    pub format_id: String,
    pub ext: String,
    pub resolution: String,
    pub height: Option<u32>,
    pub fps: Option<f32>,
    pub vcodec: String,
    pub acodec: String,
    pub filesize_approx: Option<u64>,
    pub note: String,
    pub is_audio_only: bool,
}

impl FormatOption {
    pub fn display_label(&self) -> String {
        let size_str = match self.filesize_approx {
            Some(bytes) if bytes > 0 => {
                format!(" (~{:.1} MB)", bytes as f64 / (1024.0 * 1024.0))
            }
            _ => String::new(),
        };
        if self.is_audio_only {
            format!(
                "Audio {} [{}] - {}{}",
                self.ext.to_uppercase(),
                self.acodec,
                self.note,
                size_str
            )
        } else {
            let fps_str = self
                .fps
                .filter(|&f| f > 30.0)
                .map(|f| format!(" {:.0}fps", f))
                .unwrap_or_default();
            format!(
                "{}{} ({}) [{}]{}",
                self.resolution,
                fps_str,
                self.ext.to_uppercase(),
                self.vcodec,
                size_str
            )
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistEntry {
    pub index: usize,
    pub id: String,
    pub title: String,
    pub url: String,
    pub duration_str: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMetadata {
    pub url: String,
    pub title: String,
    pub uploader: String,
    pub duration_secs: Option<u64>,
    pub thumbnail_url: Option<String>,
    pub extractor: String,
    pub webpage_url: String,
    pub is_playlist: bool,
    pub formats: Vec<FormatOption>,
    pub playlist_entries: Vec<PlaylistEntry>,
}

impl VideoMetadata {
    pub fn formatted_duration(&self) -> String {
        match self.duration_secs {
            Some(secs) if secs > 0 => {
                let hours = secs / 3600;
                let mins = (secs % 3600) / 60;
                let rem = secs % 60;
                if hours > 0 {
                    format!("{:02}:{:02}:{:02}", hours, mins, rem)
                } else {
                    format!("{:02}:{:02}", mins, rem)
                }
            }
            _ => "Live / Short".to_string(),
        }
    }

    pub fn platform_badge(&self) -> String {
        let lower = self.extractor.to_lowercase();
        let url_lower = self.url.to_lowercase();
        if lower.contains("youtube") || url_lower.contains("youtu") {
            if url_lower.contains("/shorts/") {
                "YouTube Shorts".to_string()
            } else if self.is_playlist {
                "YouTube Playlist".to_string()
            } else {
                "YouTube".to_string()
            }
        } else if lower.contains("instagram") || url_lower.contains("instagram.com") {
            if url_lower.contains("/reel") {
                "Instagram Reel".to_string()
            } else {
                "Instagram".to_string()
            }
        } else if lower.contains("facebook") || url_lower.contains("facebook.com") || url_lower.contains("fb.watch") {
            if url_lower.contains("/reel") {
                "Facebook Reel".to_string()
            } else {
                "Facebook".to_string()
            }
        } else if lower.contains("tiktok") || url_lower.contains("tiktok.com") {
            "TikTok".to_string()
        } else if lower.contains("twitter") || lower.contains("x") || url_lower.contains("x.com") || url_lower.contains("twitter.com") {
            "X / Twitter".to_string()
        } else if !self.extractor.is_empty() {
            self.extractor.clone()
        } else {
            "Web Video".to_string()
        }
    }
}

pub fn detect_platform_from_url(url: &str) -> String {
    let u = url.to_lowercase();
    if u.contains("youtube.com/shorts/") {
        "YouTube Shorts".to_string()
    } else if u.contains("youtube.com/playlist") || u.contains("&list=") || u.contains("?list=") {
        "YouTube Playlist".to_string()
    } else if u.contains("youtube.com") || u.contains("youtu.be") {
        "YouTube".to_string()
    } else if u.contains("instagram.com/reel") {
        "Instagram Reel".to_string()
    } else if u.contains("instagram.com") {
        "Instagram".to_string()
    } else if u.contains("facebook.com/reel") {
        "Facebook Reel".to_string()
    } else if u.contains("facebook.com") || u.contains("fb.watch") || u.contains("fb.gg") {
        "Facebook".to_string()
    } else if u.contains("tiktok.com") {
        "TikTok".to_string()
    } else if u.contains("x.com") || u.contains("twitter.com") {
        "X / Twitter".to_string()
    } else {
        "Media URL".to_string()
    }
}

pub fn is_likely_playlist_url(url: &str) -> bool {
    let u = url.to_lowercase();
    u.contains("youtube.com/playlist") || u.contains("&list=") || u.contains("?list=")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadStatus {
    Queued,
    Downloading,
    PostProcessing,
    RetryingBypass(u8),
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub platform: String,
    pub preset: QualityPreset,
    pub output_dir: PathBuf,
    pub playlist_items: Option<String>,
    pub expected_playlist_count: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum DownloadProgressEvent {
    Started {
        task_id: u64,
        status_text: String,
    },
    TitleUpdated {
        task_id: u64,
        title: String,
    },
    PlaylistDetected {
        task_id: u64,
        playlist_title: String,
        total_items: u32,
    },
    PlaylistItemStarted {
        task_id: u64,
        current_index: u32,
        total_items: u32,
        video_title: String,
        playlist_title: Option<String>,
    },
    PlaylistItemCompleted {
        task_id: u64,
        current_index: u32,
        total_items: u32,
        video_title: String,
        file_path: PathBuf,
    },
    Progress {
        task_id: u64,
        fraction: f64,
        speed: String,
        eta: String,
        status_text: String,
    },
    BypassRetry {
        task_id: u64,
        attempt: u8,
        strategy_name: String,
    },
    PostProcessing {
        task_id: u64,
        step: String,
    },
    Completed {
        task_id: u64,
        title: String,
        file_path: PathBuf,
    },
    Failed {
        task_id: u64,
        error: String,
    },
    Cancelled {
        task_id: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedItem {
    pub id: u64,
    pub title: String,
    pub url: String,
    pub platform: String,
    pub quality_badge: String,
    pub file_path: PathBuf,
    pub completed_at: String,
}

impl CompletedItem {
    pub const MAX_HISTORY_ROWS: usize = 10;

    /// 1. Removes any history entry whose file no longer exists on disk (`file_path.exists()`).
    /// 2. If more than `MAX_HISTORY_ROWS` (10) entries remain, deletes the oldest entries
    ///    so only the 10 most recent existing items are kept.
    pub fn prune_missing_and_cap(items: &mut Vec<Self>) -> bool {
        let initial_len = items.len();
        items.retain(|item| item.file_path.exists());
        if items.len() > Self::MAX_HISTORY_ROWS {
            let excess = items.len() - Self::MAX_HISTORY_ROWS;
            items.drain(0..excess);
        }
        items.len() != initial_len
    }

    pub fn load_history() -> Vec<Self> {
        let path = metadata::history_file_path();
        if let Ok(content) = std::fs::read_to_string(path) {
            let clean = content.trim_start_matches('\u{feff}');
            if let Ok(mut list) = serde_json::from_str::<Vec<CompletedItem>>(clean) {
                if Self::prune_missing_and_cap(&mut list) {
                    Self::save_history(&list);
                }
                return list;
            }
        }
        Vec::new()
    }

    pub fn push_and_save(items: &mut Vec<Self>, new_item: Self) {
        items.push(new_item);
        Self::prune_missing_and_cap(items);
        Self::save_history(items);
    }

    pub fn save_history(items: &[Self]) {
        let path = metadata::history_file_path();
        if let Ok(content) = serde_json::to_string_pretty(items) {
            let _ = std::fs::write(path, content);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_prune_missing_and_cap_at_10() {
        let temp_dir = std::env::temp_dir().join("omni_history_test");
        let _ = std::fs::create_dir_all(&temp_dir);

        let mut list = Vec::new();
        // Add 3 non-existent files
        for i in 1..=3 {
            list.push(CompletedItem {
                id: i,
                title: format!("Missing {}", i),
                url: "https://example.com".to_string(),
                platform: "YouTube".to_string(),
                quality_badge: "1080p MP4".to_string(),
                file_path: temp_dir.join(format!("missing_{}.mp4", i)),
                completed_at: "Completed".to_string(),
            });
        }

        // Create 13 real files on disk
        for i in 1..=13 {
            let fp = temp_dir.join(format!("existing_{}.mp4", i));
            let _ = std::fs::write(&fp, b"video");
            list.push(CompletedItem {
                id: 100 + i,
                title: format!("Existing {}", i),
                url: "https://example.com".to_string(),
                platform: "YouTube".to_string(),
                quality_badge: "1080p MP4".to_string(),
                file_path: fp,
                completed_at: "Completed".to_string(),
            });
        }

        let changed = CompletedItem::prune_missing_and_cap(&mut list);
        assert!(changed);
        assert_eq!(list.len(), 10, "Should cap history at 10 existing items");
        // Should keep items 4..=13 (the 10 newest existing files)
        assert_eq!(list.first().unwrap().title, "Existing 4");
        assert_eq!(list.last().unwrap().title, "Existing 13");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
