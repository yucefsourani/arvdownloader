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

pub fn resolve_firefox_cookie_spec(home: &std::path::Path) -> Option<String> {
    let mut base_dirs = vec![
        home.join(".mozilla/firefox"),
        home.join(".var/app/org.mozilla.firefox/.mozilla/firefox"),
        home.join(".var/app/org.mozilla.firefox/config/mozilla/firefox"),
        home.join("snap/firefox/common/.mozilla/firefox"),
        home.join(".config/mozilla/firefox"),
        home.join("AppData/Roaming/Mozilla/Firefox/Profiles"),
        home.join("AppData/Roaming/Mozilla/Firefox"),
        home.join("AppData/Local/Mozilla/Firefox/Profiles"),
        home.join("AppData/Local/Mozilla/Firefox"),
        home.join("scoop/persist/firefox/profile"),
        home.join("scoop/apps/firefox/current/profile"),
    ];

    if let Some(appdata) = std::env::var_os("APPDATA") {
        let p = PathBuf::from(appdata);
        base_dirs.push(p.join("Mozilla/Firefox/Profiles"));
        base_dirs.push(p.join("Mozilla/Firefox"));
    }

    let mut packages_roots = vec![home.join("AppData/Local/Packages")];
    if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA") {
        let p = PathBuf::from(local_appdata);
        base_dirs.push(p.join("Mozilla/Firefox/Profiles"));
        base_dirs.push(p.join("Mozilla/Firefox"));
        packages_roots.push(p.join("Packages"));
    }

    // Scan Windows Microsoft Store (MSIX) packages for Firefox (e.g. Mozilla.Firefox_n80bbvh6b1yt2)
    for pkg_root in packages_roots {
        append_ms_store_firefox_dirs(&pkg_root, &mut base_dirs);
    }

    resolve_firefox_cookie_spec_from_dirs(&base_dirs)
}

pub fn append_ms_store_firefox_dirs(pkg_root: &std::path::Path, base_dirs: &mut Vec<PathBuf>) {
    if !pkg_root.exists() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(pkg_root) else {
        return;
    };
    for entry in entries.flatten() {
        let pkg_path = entry.path();
        if !pkg_path.is_dir() {
            continue;
        }
        let folder_name = entry.file_name().to_string_lossy().to_lowercase();
        if folder_name.contains("firefox") || folder_name.contains("mozilla") {
            for rel in [
                "LocalCache/Roaming/Mozilla/Firefox/Profiles",
                "LocalCache/Roaming/Mozilla/Firefox",
                "LocalCache/Local/Mozilla/Firefox/Profiles",
                "LocalCache/Local/Mozilla/Firefox",
                "RoamingState/Mozilla/Firefox/Profiles",
                "RoamingState/Mozilla/Firefox",
                "LocalState/Mozilla/Firefox/Profiles",
                "LocalState/Mozilla/Firefox",
            ] {
                base_dirs.push(pkg_path.join(rel));
            }
        }
    }
}

pub fn resolve_firefox_cookie_spec_from_dirs(base_dirs: &[PathBuf]) -> Option<String> {
    let mut candidate_profiles = Vec::new();
    for base in base_dirs {
        if !base.exists() || !base.is_dir() {
            continue;
        }
        candidate_profiles.push(base.clone());
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    candidate_profiles.push(path.clone());
                    // Also check nested Profiles/ directory if `base` was `Mozilla/Firefox`
                    if path
                        .file_name()
                        .map(|n| n.to_string_lossy().eq_ignore_ascii_case("Profiles"))
                        .unwrap_or(false)
                    {
                        if let Ok(sub_entries) = std::fs::read_dir(&path) {
                            for sub in sub_entries.flatten() {
                                let sub_path = sub.path();
                                if sub_path.is_dir() {
                                    candidate_profiles.push(sub_path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut best_profile: Option<(PathBuf, std::time::SystemTime, u64)> = None;
    let mut fallback_profile: Option<(PathBuf, std::time::SystemTime)> = None;

    for path in candidate_profiles {
        let cookie_db = path.join("cookies.sqlite");
        let cookie_wal = path.join("cookies.sqlite-wal");

        let db_meta = std::fs::metadata(&cookie_db).ok();
        let wal_meta = std::fs::metadata(&cookie_wal).ok();

        let db_len = db_meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let wal_len = wal_meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let total_len = db_len.saturating_add(wal_len);

        if total_len > 0 {
            let db_mod = db_meta
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::UNIX_EPOCH);
            let wal_mod = wal_meta
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::UNIX_EPOCH);
            let modified = db_mod.max(wal_mod);

            let should_replace = match &best_profile {
                None => true,
                Some((_, best_mod, best_len)) => (modified, total_len) > (*best_mod, *best_len),
            };
            if should_replace {
                best_profile = Some((path, modified, total_len));
            }
            continue;
        }

        // Fallback for freshly installed Microsoft Store / Windows Firefox profile before first cookie write
        let prefs_js = path.join("prefs.js");
        let places_db = path.join("places.sqlite");
        let marker_meta = db_meta
            .or_else(|| std::fs::metadata(&prefs_js).ok())
            .or_else(|| std::fs::metadata(&places_db).ok());

        if let Some(meta) = marker_meta {
            let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            let should_replace = match &fallback_profile {
                None => true,
                Some((_, best_mod)) => modified > *best_mod,
            };
            if should_replace {
                fallback_profile = Some((path, modified));
            }
        }
    }

    if let Some((profile_dir, _, _)) = best_profile {
        return Some(format!(
            "firefox:{}",
            metadata::normalize_path(&profile_dir).to_string_lossy()
        ));
    }

    fallback_profile.map(|(profile_dir, _)| {
        format!(
            "firefox:{}",
            metadata::normalize_path(&profile_dir).to_string_lossy()
        )
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
    #[serde(default = "default_true")]
    pub dark_theme: bool,
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
            dark_theme: true,
        }
    }
}

impl AppSettings {
    pub fn load() -> Self {
        let path = metadata::config_file_path();
        if !path.exists() {
            if let Some(legacy_dir) = metadata::legacy_app_home_dir() {
                let legacy_cfg = legacy_dir.join("config.json");
                if legacy_cfg.exists() {
                    let _ = std::fs::copy(&legacy_cfg, &path);
                }
            }
        }
        if let Ok(data) = std::fs::read_to_string(&path) {
            let clean = data.trim_start_matches('\u{feff}');
            if let Ok(mut cfg) = serde_json::from_str::<AppSettings>(clean) {
                let is_legacy_omni = cfg
                    .download_dir
                    .to_string_lossy()
                    .to_lowercase()
                    .contains("omni");
                if is_legacy_omni || cfg.download_dir.as_os_str().is_empty() {
                    cfg.download_dir = metadata::default_download_dir();
                    cfg.save();
                } else {
                    let _ = std::fs::create_dir_all(&cfg.download_dir);
                    cfg.download_dir = metadata::normalize_path(&cfg.download_dir);
                }
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
        if !path.exists() {
            if let Some(legacy_dir) = metadata::legacy_app_home_dir() {
                let legacy_ver = legacy_dir.join("bin").join("versions.json");
                if legacy_ver.exists() {
                    let _ = std::fs::copy(&legacy_ver, &path);
                }
            }
        }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub platform: String,
    pub preset: QualityPreset,
    pub output_dir: PathBuf,
    pub playlist_items: Option<String>,
    pub expected_playlist_count: Option<u32>,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub thumbnail_path: Option<PathBuf>,
    #[serde(default)]
    pub saved_fraction: f64,
    #[serde(default)]
    pub saved_status: String,
    /// Files of playlist videos already finished (and added to Completed History) by this task.
    /// Persisted in `queue.json` so stopping/resuming never duplicates history entries.
    #[serde(default)]
    pub completed_files: Vec<PathBuf>,
}

impl DownloadTask {
    /// Returns the existing local thumbnail file path on disk (checking `thumbnail_path` first,
    /// then the deterministic `thumbnails_dir()/task_<id>.jpg` path).
    pub fn resolved_thumbnail_path(&self) -> Option<PathBuf> {
        if let Some(ref custom) = self.thumbnail_path {
            if custom.is_file() {
                return Some(custom.clone());
            }
        }
        let standard = metadata::task_thumbnail_file_path(self.id);
        if standard.is_file() {
            return Some(standard);
        }
        None
    }

    pub fn load_saved_queue() -> Vec<Self> {
        let path = metadata::queue_file_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            let clean = content.trim_start_matches('\u{feff}');
            if let Ok(mut list) = serde_json::from_str::<Vec<DownloadTask>>(clean) {
                for task in &mut list {
                    let expected_thumb = metadata::task_thumbnail_file_path(task.id);
                    if expected_thumb.exists() {
                        task.thumbnail_path = Some(expected_thumb);
                    } else if let Some(ref p) = task.thumbnail_path {
                        if !p.exists() {
                            task.thumbnail_path = None;
                        }
                    }
                }
                return list;
            }
        }
        Vec::new()
    }

    pub fn save_queue(tasks: &[Self]) {
        let path = metadata::queue_file_path();
        if let Ok(content) = serde_json::to_string_pretty(tasks) {
            let _ = std::fs::write(path, content);
        }
    }

    /// Removes any leftover files in `thumbnails_dir()` that belong neither to an active/saved
    /// queue item nor to a Completed History item.
    pub fn cleanup_orphan_thumbnails(active_tasks: &[Self], history: &[CompletedItem]) {
        let thumb_dir = metadata::thumbnails_dir();
        let Ok(entries) = std::fs::read_dir(&thumb_dir) else {
            return;
        };
        for entry in entries.flatten() {
            let file_path = entry.path();
            if !file_path.is_file() {
                continue;
            }
            let keep_queue = active_tasks.iter().any(|t| {
                metadata::task_thumbnail_file_path(t.id) == file_path
                    || t.thumbnail_path.as_deref() == Some(file_path.as_path())
            });
            let keep_history = history
                .iter()
                .any(|h| h.thumbnail_path.as_deref() == Some(file_path.as_path()));
            if !keep_queue && !keep_history {
                let _ = std::fs::remove_file(&file_path);
            }
        }
    }
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
    ThumbnailUpdated {
        task_id: u64,
        thumbnail_url: String,
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
    /// Original remote thumbnail URL (backup used to re-download the image if the local file is missing)
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    /// Local thumbnail file stored in `thumbnails_dir()` (`history_<id>_<millis>.jpg`)
    #[serde(default)]
    pub thumbnail_path: Option<PathBuf>,
}

impl CompletedItem {
    pub const MAX_HISTORY_ROWS: usize = 10;

    /// Returns the local thumbnail path only if the file still exists on disk
    pub fn resolved_thumbnail_path(&self) -> Option<PathBuf> {
        self.thumbnail_path
            .as_ref()
            .filter(|p| p.is_file())
            .cloned()
    }

    /// Deletes this history item's thumbnail file from `thumbnails_dir()`
    pub fn delete_thumbnail(&self) {
        metadata::delete_thumbnail_file(self.thumbnail_path.as_deref());
    }

    /// 1. Removes any history entry whose file no longer exists on disk (`file_path.exists()`).
    /// 2. If more than `MAX_HISTORY_ROWS` (10) entries remain, deletes the oldest entries
    ///    so only the 10 most recent existing items are kept.
    ///
    /// The thumbnail files of every removed entry are deleted from `thumbnails_dir()`.
    pub fn prune_missing_and_cap(items: &mut Vec<Self>) -> bool {
        let initial_len = items.len();
        let mut removed: Vec<Self> = Vec::new();
        let mut kept: Vec<Self> = Vec::with_capacity(items.len());
        for item in items.drain(..) {
            if item.file_path.exists() {
                kept.push(item);
            } else {
                removed.push(item);
            }
        }
        if kept.len() > Self::MAX_HISTORY_ROWS {
            let excess = kept.len() - Self::MAX_HISTORY_ROWS;
            removed.extend(kept.drain(0..excess));
        }
        *items = kept;

        for old in &removed {
            let still_used = old.thumbnail_path.is_some()
                && items
                    .iter()
                    .any(|i| i.thumbnail_path.is_some() && i.thumbnail_path == old.thumbnail_path);
            if !still_used {
                old.delete_thumbnail();
            }
        }
        items.len() != initial_len
    }

    /// Explicitly removes a single history entry (matched by id + file path) and deletes its thumbnail
    pub fn remove_and_save(items: &mut Vec<Self>, id: u64, file_path: &std::path::Path) {
        Self::remove_entry(items, id, file_path);
        Self::save_history(items);
    }

    /// Removes a single entry (matched by id + file path) from memory and deletes its thumbnail file
    pub fn remove_entry(items: &mut Vec<Self>, id: u64, file_path: &std::path::Path) -> bool {
        if let Some(pos) = items
            .iter()
            .position(|i| i.id == id && i.file_path == file_path)
        {
            let removed = items.remove(pos);
            removed.delete_thumbnail();
            return true;
        }
        false
    }

    /// Explicitly clears the whole history and deletes all history thumbnails
    pub fn clear_and_save(items: &mut Vec<Self>) {
        for item in items.iter() {
            item.delete_thumbnail();
        }
        items.clear();
        Self::save_history(items);
    }

    pub fn load_history() -> Vec<Self> {
        let path = metadata::history_file_path();
        if !path.exists() {
            if let Some(legacy_dir) = metadata::legacy_app_home_dir() {
                let legacy_hist = legacy_dir.join("history.json");
                if legacy_hist.exists() {
                    let _ = std::fs::copy(&legacy_hist, &path);
                }
            }
        }
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
        Self::upsert(items, new_item);
        Self::prune_missing_and_cap(items);
        Self::save_history(items);
    }

    /// Adds `new_item` as the newest entry. If an entry for the same file already exists
    /// (e.g. a resumed playlist re-reporting a finished video, or the same video downloaded
    /// again to the same file) the old entry is replaced instead of duplicated, keeping its
    /// thumbnail when the new entry has none.
    pub fn upsert(items: &mut Vec<Self>, mut new_item: Self) {
        let same_file = |a: &std::path::Path, b: &std::path::Path| {
            if a == b {
                return true;
            }
            match (a.canonicalize(), b.canonicalize()) {
                (Ok(ca), Ok(cb)) => ca == cb,
                _ => false,
            }
        };
        let mut idx = 0;
        while idx < items.len() {
            if same_file(&items[idx].file_path, &new_item.file_path) {
                let old = items.remove(idx);
                if new_item.resolved_thumbnail_path().is_none() {
                    if let Some(old_thumb) = old.resolved_thumbnail_path() {
                        new_item.thumbnail_path = Some(old_thumb);
                    }
                    if new_item.thumbnail_url.is_none() {
                        new_item.thumbnail_url = old.thumbnail_url.clone();
                    }
                }
                if old.thumbnail_path.is_some() && old.thumbnail_path != new_item.thumbnail_path {
                    old.delete_thumbnail();
                }
            } else {
                idx += 1;
            }
        }
        items.push(new_item);
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
    fn test_windows_ms_store_firefox_cookie_detection() {
        let temp_root = std::env::temp_dir().join("arv_ms_store_firefox_test");
        let _ = std::fs::remove_dir_all(&temp_root);

        let pkg_root = temp_root.join("Packages");
        let profile_dir = pkg_root
            .join("Mozilla.Firefox_n80bbvh6b1yt2")
            .join("LocalCache")
            .join("Roaming")
            .join("Mozilla")
            .join("Firefox")
            .join("Profiles")
            .join("abc12345.default-release");

        std::fs::create_dir_all(&profile_dir).unwrap();
        std::fs::write(profile_dir.join("cookies.sqlite"), b"SQLite format 3\0").unwrap();

        let mut base_dirs = Vec::new();
        append_ms_store_firefox_dirs(&pkg_root, &mut base_dirs);
        let spec = resolve_firefox_cookie_spec_from_dirs(&base_dirs);

        assert!(spec.is_some(), "Should detect Microsoft Store Firefox profile");
        let spec_str = spec.unwrap();
        assert!(spec_str.starts_with("firefox:"));
        assert!(spec_str.contains("abc12345.default-release"));

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_dark_theme_enabled_by_default_and_persisted() {
        let default_settings = AppSettings::default();
        assert!(
            default_settings.dark_theme,
            "Dark theme should be enabled by default"
        );

        let mut modified = default_settings.clone();
        modified.dark_theme = false;
        let json = serde_json::to_string(&modified).unwrap();
        let restored: AppSettings = serde_json::from_str(&json).unwrap();
        assert!(
            !restored.dark_theme,
            "Saved light theme state should be restored accurately"
        );
    }

    #[test]
    fn test_history_prune_missing_and_cap_at_10() {
        let temp_dir = std::env::temp_dir().join("arv_history_test");
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
                thumbnail_url: None,
                thumbnail_path: None,
            });
        }

        // Create 13 real files on disk
        for i in 1..=13 {
            let fp = temp_dir.join(format!("existing_{}.mp4", i));
            let _ = std::fs::write(&fp, b"video");
            let thumb = metadata::new_history_thumbnail_file_path(900_000 + i);
            let _ = std::fs::write(&thumb, b"jpg");
            list.push(CompletedItem {
                id: 100 + i,
                title: format!("Existing {}", i),
                url: "https://example.com".to_string(),
                platform: "YouTube".to_string(),
                quality_badge: "1080p MP4".to_string(),
                file_path: fp,
                completed_at: "Completed".to_string(),
                thumbnail_url: Some("https://i.ytimg.com/vi/x/hqdefault.jpg".to_string()),
                thumbnail_path: Some(thumb),
            });
        }
        let oldest_thumbs: Vec<PathBuf> = list[3..6]
            .iter()
            .filter_map(|i| i.thumbnail_path.clone())
            .collect();

        let changed = CompletedItem::prune_missing_and_cap(&mut list);
        assert!(changed);
        assert_eq!(list.len(), 10, "Should cap history at 10 existing items");
        // Should keep items 4..=13 (the 10 newest existing files)
        assert_eq!(list.first().unwrap().title, "Existing 4");
        assert_eq!(list.last().unwrap().title, "Existing 13");
        for t in &oldest_thumbs {
            assert!(!t.exists(), "Thumbnails of pruned oldest items must be deleted");
        }
        for item in &list {
            assert!(item.resolved_thumbnail_path().is_some(), "Kept items keep thumbnails");
        }

        // Explicit removal of one row deletes only its thumbnail
        let first = list[0].clone();
        CompletedItem::remove_entry(&mut list, first.id, &first.file_path);
        assert_eq!(list.len(), 9);
        assert!(!first.thumbnail_path.unwrap().exists());

        // Explicit clear deletes all thumbnails
        let remaining: Vec<PathBuf> = list.iter().filter_map(|i| i.thumbnail_path.clone()).collect();
        for item in list.iter() {
            item.delete_thumbnail();
        }
        list.clear();
        for t in remaining {
            assert!(!t.exists());
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_history_upsert_prevents_duplicates_on_resume() {
        let temp_dir = std::env::temp_dir().join("arv_history_upsert_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let fp = temp_dir.join("video_1.mp4");
        std::fs::write(&fp, b"video").unwrap();
        let other = temp_dir.join("video_2.mp4");
        std::fs::write(&other, b"video").unwrap();

        let thumb = metadata::new_history_thumbnail_file_path(777_001);
        std::fs::write(&thumb, b"jpg").unwrap();

        let make = |id: u64, path: &PathBuf, thumb: Option<PathBuf>| CompletedItem {
            id,
            title: format!("Item {}", id),
            url: "https://www.youtube.com/playlist?list=PL123".to_string(),
            platform: "YouTube".to_string(),
            quality_badge: "1080p MP4".to_string(),
            file_path: path.clone(),
            completed_at: "Completed".to_string(),
            thumbnail_url: Some("https://i.ytimg.com/vi/a/hqdefault.jpg".to_string()),
            thumbnail_path: thumb,
        };

        let mut list = Vec::new();
        CompletedItem::upsert(&mut list, make(1001, &fp, Some(thumb.clone())));
        CompletedItem::upsert(&mut list, make(1002, &other, None));
        // Resume re-reports the first video with a new id and no local thumbnail
        CompletedItem::upsert(&mut list, make(5001, &fp, None));

        assert_eq!(list.len(), 2, "Same file must not be duplicated in history");
        assert_eq!(list.last().unwrap().id, 5001);
        assert_eq!(
            list.last().unwrap().thumbnail_path.as_ref(),
            Some(&thumb),
            "Existing thumbnail should be kept for the replaced entry"
        );
        assert!(thumb.exists());

        let _ = std::fs::remove_file(&thumb);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_queue_persistence_and_thumbnail_cleanup() {
        let task_id = 999_888_777_u64;
        let thumb_path = metadata::task_thumbnail_file_path(task_id);
        std::fs::write(&thumb_path, b"fake_jpg_bytes").unwrap();
        assert!(thumb_path.is_file());

        let task = DownloadTask {
            id: task_id,
            url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
            title: "Test Resumable Video".to_string(),
            platform: "YouTube".to_string(),
            preset: QualityPreset::Video1080p,
            output_dir: metadata::default_download_dir(),
            playlist_items: None,
            expected_playlist_count: None,
            thumbnail_url: Some(
                "https://i.ytimg.com/vi_webp/dQw4w9WgXcQ/maxresdefault.webp".to_string(),
            ),
            thumbnail_path: Some(thumb_path.clone()),
            saved_fraction: 0.42,
            saved_status: "Stopped — Click Resume to continue download".to_string(),
            completed_files: Vec::new(),
        };

        assert_eq!(
            metadata::normalize_thumbnail_url(task.thumbnail_url.as_ref().unwrap()),
            "https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg"
        );
        assert_eq!(task.resolved_thumbnail_path(), Some(thumb_path.clone()));

        let json = serde_json::to_string(&vec![task]).unwrap();
        let restored: Vec<DownloadTask> = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].id, task_id);
        assert!((restored[0].saved_fraction - 0.42).abs() < 1e-6);

        metadata::delete_task_thumbnail(task_id, restored[0].thumbnail_path.as_deref());
        assert!(
            !thumb_path.exists(),
            "Thumbnail file in thumbnails/ folder should be deleted on completion or removal"
        );
    }
}
