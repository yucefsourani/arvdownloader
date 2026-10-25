use crate::metadata;
use crate::models::{
    AppSettings, BrowserCookieSource, DownloadProgressEvent, DownloadTask, FormatOption,
    PlaylistEntry, QualityPreset, VideoMetadata,
};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

static CANCEL_TOKENS: Lazy<Mutex<HashMap<u64, Arc<AtomicBool>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn register_cancel_token(task_id: u64) -> Arc<AtomicBool> {
    let token = Arc::new(AtomicBool::new(false));
    if let Ok(mut map) = CANCEL_TOKENS.lock() {
        map.insert(task_id, token.clone());
    }
    token
}

pub fn cancel_task(task_id: u64) {
    if let Ok(map) = CANCEL_TOKENS.lock() {
        if let Some(token) = map.get(&task_id) {
            token.store(true, Ordering::SeqCst);
        }
    }
}

pub fn remove_cancel_token(task_id: u64) {
    if let Ok(mut map) = CANCEL_TOKENS.lock() {
        map.remove(&task_id);
    }
}

pub fn has_active_downloads() -> bool {
    if let Ok(map) = CANCEL_TOKENS.lock() {
        map.values().any(|token| !token.load(Ordering::SeqCst))
    } else {
        false
    }
}

pub fn cancel_all_tasks() {
    if let Ok(map) = CANCEL_TOKENS.lock() {
        for token in map.values() {
            token.store(true, Ordering::SeqCst);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BypassAttempt {
    /// Attempt 1: Standard extractor + active browser cookies + JS runtime solver + geo-bypass
    StandardWithCookies,
    /// Attempt 2: Authenticated Web/TV/MWeb client bypass + active cookies + JS runtime solver
    MobileClientBypass,
    /// Attempt 3: Cookie-free iOS/Android/TV/Safari client fallback (in case browser DB is locked or cookies expired)
    EmbeddedTvFallback,
}

impl BypassAttempt {
    pub fn strategy_label(&self) -> &'static str {
        match self {
            BypassAttempt::StandardWithCookies => "Standard + Active Cookies + JS Solver",
            BypassAttempt::MobileClientBypass => "Authenticated Web/TV Client Bypass",
            BypassAttempt::EmbeddedTvFallback => "Cookie-Free Mobile/TV Client Fallback",
        }
    }
}

/// Resolves the `yt-dlp` binary path, prioritizing `~/.video_downloader/bin/yt-dlp(.exe)`.
pub fn resolve_ytdlp_executable() -> PathBuf {
    let managed = metadata::managed_ytdlp_path();
    if managed.exists() {
        return managed;
    }
    PathBuf::from(metadata::ytdlp_bin_name())
}

/// Resolves the directory containing `ffmpeg` if managed in `~/.video_downloader/bin`.
pub fn resolve_ffmpeg_dir() -> Option<PathBuf> {
    let managed_ffmpeg = metadata::managed_ffmpeg_path();
    if managed_ffmpeg.exists() {
        return Some(metadata::managed_bin_dir());
    }
    None
}

/// Resolves the best available browser cookie specification (`BROWSER` or `BROWSER:PROFILE_PATH`).
/// If the user selected a browser that is not installed on this machine, automatically falls back
/// to the first detected installed browser so `yt-dlp` never fails with "could not find cookies database".
pub fn resolve_effective_cookie_spec(source: BrowserCookieSource) -> Option<String> {
    if let Some(spec) = source.resolve_cookie_browser_spec() {
        return Some(spec);
    }
    // Fallback if user picked an uninstalled browser
    for fallback in [
        BrowserCookieSource::Firefox,
        BrowserCookieSource::Brave,
        BrowserCookieSource::Chrome,
        BrowserCookieSource::Edge,
        BrowserCookieSource::Chromium,
        BrowserCookieSource::Vivaldi,
        BrowserCookieSource::Opera,
    ] {
        if let Some(spec) = fallback.resolve_cookie_browser_spec() {
            return Some(spec);
        }
    }
    None
}

/// Configures Windows `CREATE_NO_WINDOW` and forces UTF-8 I/O environment variables across
/// Windows and Linux so `yt-dlp(.exe)`, `ffmpeg(.exe)`, and `deno(.exe)` never corrupt
/// Arabic or Unicode filenames/titles when `stdout`/`stderr` are piped.
pub fn configure_command_platform(cmd: &mut Command) {
    cmd.env("PYTHONUTF8", "1");
    cmd.env("PYTHONIOENCODING", "utf-8:replace");
    cmd.env("LANG", "C.UTF-8");
    cmd.env("LC_ALL", "C.UTF-8");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
}

/// Appends anti-block, network resilience, JS runtime (EJS solver), cookie, and site-specific arguments to `yt-dlp`.
pub fn apply_bypass_and_network_args(
    args: &mut Vec<String>,
    url: &str,
    settings: &AppSettings,
    attempt: BypassAttempt,
) {
    args.push("--encoding".to_string());
    args.push("utf-8".to_string());
    args.push("--no-colors".to_string());
    args.push("--geo-bypass".to_string());
    args.push("--no-check-certificates".to_string());
    args.push("--retries".to_string());
    args.push("10".to_string());
    args.push("--fragment-retries".to_string());
    args.push("10".to_string());
    args.push("--retry-sleep".to_string());
    args.push("linear=1::2".to_string());
    args.push("--concurrent-fragments".to_string());
    args.push("4".to_string());

    // Enable JavaScript runtimes (deno, node, quickjs, bun) required by yt-dlp 2026+ EJS signature solving:
    // - If `settings.use_managed_deno` is true (default), prioritize `~/.video_downloader/bin/deno` first.
    // - If `settings.use_managed_deno` is false, prioritize system PATH binaries (`deno`, `node`, `quickjs`, `bun`).
    let managed_deno = metadata::managed_deno_path();
    if settings.use_managed_deno && managed_deno.exists() {
        args.push("--js-runtimes".to_string());
        args.push(format!("deno:{}", managed_deno.to_string_lossy()));
        args.push("--js-runtimes".to_string());
        args.push("node".to_string());
        args.push("--js-runtimes".to_string());
        args.push("quickjs".to_string());
        args.push("--js-runtimes".to_string());
        args.push("bun".to_string());
    } else {
        args.push("--js-runtimes".to_string());
        args.push("deno".to_string());
        args.push("--js-runtimes".to_string());
        args.push("node".to_string());
        args.push("--js-runtimes".to_string());
        args.push("quickjs".to_string());
        args.push("--js-runtimes".to_string());
        args.push("bun".to_string());
    }

    if let Some(ffmpeg_dir) = resolve_ffmpeg_dir() {
        args.push("--ffmpeg-location".to_string());
        args.push(ffmpeg_dir.to_string_lossy().to_string());
    }

    if !settings.proxy_url.trim().is_empty() {
        args.push("--proxy".to_string());
        args.push(settings.proxy_url.trim().to_string());
    }

    let mut using_cookies = false;

    // Apply cookies on Attempt 1 and Attempt 2 (on Attempt 3 we run cookie-free in case browser DB is locked on Windows)
    if attempt != BypassAttempt::EmbeddedTvFallback {
        match settings.cookie_source {
            BrowserCookieSource::None => {}
            BrowserCookieSource::CustomFile => {
                if let Some(ref path) = settings.custom_cookies_path {
                    if path.exists() {
                        args.push("--cookies".to_string());
                        args.push(path.to_string_lossy().to_string());
                        using_cookies = true;
                    }
                }
            }
            browser => {
                if let Some(spec) = resolve_effective_cookie_spec(browser) {
                    args.push("--cookies-from-browser".to_string());
                    args.push(spec);
                    using_cookies = true;
                }
            }
        }
    }

    let url_lower = url.to_lowercase();
    let is_youtube = url_lower.contains("youtube.com") || url_lower.contains("youtu.be");
    let is_instagram = url_lower.contains("instagram.com");
    let is_facebook = url_lower.contains("facebook.com")
        || url_lower.contains("fb.watch")
        || url_lower.contains("fb.gg");

    match attempt {
        BypassAttempt::StandardWithCookies => {
            if is_instagram || is_facebook {
                args.push("--user-agent".to_string());
                args.push(metadata::HTTP_USER_AGENT.to_string());
            }
        }
        BypassAttempt::MobileClientBypass => {
            if is_youtube {
                args.push("--extractor-args".to_string());
                if using_cookies {
                    // When cookies are active, use clients that support cookies (web, tv, mweb, web_embedded)
                    args.push("youtube:player_client=web,tv,mweb,web_embedded".to_string());
                } else {
                    args.push("youtube:player_client=ios,web,android".to_string());
                }
            } else {
                args.push("--user-agent".to_string());
                args.push(
                    "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1"
                        .to_string(),
                );
            }
            args.push("--add-header".to_string());
            args.push("Accept-Language:en-US,en;q=0.9".to_string());
        }
        BypassAttempt::EmbeddedTvFallback => {
            if is_youtube {
                args.push("--extractor-args".to_string());
                args.push("youtube:player_client=ios,android,tv,web_safari,mweb".to_string());
            } else {
                args.push("--user-agent".to_string());
                args.push(metadata::HTTP_USER_AGENT.to_string());
            }
        }
    }
}

/// Appends format selection, audio extraction, thumbnail embedding, and output template arguments.
///
/// - For **Playlists**: Saves all videos inside a dedicated subfolder named after the playlist
///   (`<output_dir>/<Playlist Name>/01 - <Video Title> [<id>].<ext>`), enforcing strict character
///   and UTF-8 byte limits so paths never exceed Linux `NAME_MAX` (255 bytes) or Windows `MAX_PATH` (260 chars).
/// - For **Single Videos/Audio**: Saves directly in `<output_dir>/<Video Title> [<id>].<ext>` with safe length limits.
pub fn apply_format_args(
    args: &mut Vec<String>,
    preset: &QualityPreset,
    settings: &AppSettings,
    output_dir: &Path,
    playlist_items: &Option<String>,
    task_title: &str,
    relax_constraints: bool,
) {
    let is_playlist = playlist_items.is_some();

    if let Some(ref items) = playlist_items {
        args.push("--yes-playlist".to_string());
        args.push("--ignore-errors".to_string());
        let trimmed = items.trim();
        if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("all") {
            args.push("--playlist-items".to_string());
            args.push(trimmed.to_string());
        }
    } else {
        args.push("--no-playlist".to_string());
    }

    match preset {
        QualityPreset::BestVideo => {
            args.push("-f".to_string());
            if relax_constraints {
                args.push("bv*+ba/b".to_string());
            } else {
                args.push("bv*[ext=mp4]+ba[ext=m4a]/bv*+ba/b[ext=mp4]/b".to_string());
            }
            args.push("--merge-output-format".to_string());
            args.push("mp4".to_string());
        }
        QualityPreset::Video1080p => {
            args.push("-f".to_string());
            args.push(
                "bv*[height<=1080][ext=mp4]+ba[ext=m4a]/bv*[height<=1080]+ba/b[height<=1080]/b"
                    .to_string(),
            );
            args.push("--merge-output-format".to_string());
            args.push("mp4".to_string());
        }
        QualityPreset::Video720p => {
            args.push("-f".to_string());
            args.push(
                "bv*[height<=720][ext=mp4]+ba[ext=m4a]/bv*[height<=720]+ba/b[height<=720]/b"
                    .to_string(),
            );
            args.push("--merge-output-format".to_string());
            args.push("mp4".to_string());
        }
        QualityPreset::Video480p => {
            args.push("-f".to_string());
            args.push(
                "bv*[height<=480][ext=mp4]+ba[ext=m4a]/bv*[height<=480]+ba/b[height<=480]/b"
                    .to_string(),
            );
            args.push("--merge-output-format".to_string());
            args.push("mp4".to_string());
        }
        QualityPreset::AudioMp3 => {
            args.push("-f".to_string());
            args.push("ba/b".to_string());
            args.push("-x".to_string());
            args.push("--audio-format".to_string());
            args.push("mp3".to_string());
            args.push("--audio-quality".to_string());
            args.push("0".to_string());
        }
        QualityPreset::AudioM4a => {
            args.push("-f".to_string());
            args.push("ba[ext=m4a]/ba/b".to_string());
            args.push("-x".to_string());
            args.push("--audio-format".to_string());
            args.push("m4a".to_string());
            args.push("--audio-quality".to_string());
            args.push("0".to_string());
        }
        QualityPreset::CustomFormat {
            format_id,
            is_audio_only,
            ..
        } => {
            args.push("-f".to_string());
            if *is_audio_only {
                args.push(format!("{}/ba/b", format_id));
            } else {
                args.push(format!("{}+ba/{}/bv*+ba/b", format_id, format_id));
                args.push("--merge-output-format".to_string());
                args.push("mp4".to_string());
            }
        }
    }

    if settings.embed_metadata {
        args.push("--embed-metadata".to_string());
    }

    if settings.embed_thumbnail && preset.is_audio() {
        args.push("--embed-thumbnail".to_string());
        args.push("--convert-thumbnails".to_string());
        args.push("jpg".to_string());
    }

    if settings.sponsorblock_remove {
        args.push("--sponsorblock-remove".to_string());
        args.push("sponsor,selfpromo,interaction".to_string());
    }

    // Safe cross-platform filename sanitation (prevents invalid Windows characters `<>:"/\|?*`,
    // trailing dots/spaces, and reserved DOS device names).
    args.push("--windows-filenames".to_string());

    let (folder_max_chars, title_max_chars) =
        metadata::compute_safe_path_limits(output_dir, is_playlist);

    if is_playlist {
        let trimmed_title = task_title
            .trim()
            .strip_prefix("Playlist:")
            .unwrap_or(task_title)
            .trim();
        let has_known_playlist_title = !trimmed_title.is_empty()
            && !trimmed_title.starts_with("Fetching ")
            && !trimmed_title.starts_with("Untitled");

        if has_known_playlist_title {
            // Create a dedicated, sanitized subfolder for the playlist with strict char & UTF-8 byte limits
            let safe_folder = metadata::sanitize_fs_name(
                trimmed_title,
                folder_max_chars,
                folder_max_chars * 3,
            );
            let playlist_dir = output_dir.join(safe_folder);
            let _ = std::fs::create_dir_all(&playlist_dir);
            let norm_playlist_dir = metadata::normalize_path(&playlist_dir);

            args.push("-P".to_string());
            args.push(norm_playlist_dir.to_string_lossy().to_string());
            args.push("-o".to_string());
            args.push(format!(
                "%(playlist_index&{{:02d}} - |)s%(title).{}s [%(id)s].%(ext)s",
                title_max_chars
            ));
        } else {
            // Quick Download on a playlist URL before title is inspected:
            // yt-dlp creates the dedicated subfolder `<output_dir>/<playlist_title>/` capped to `folder_max_chars`
            args.push("-P".to_string());
            args.push(output_dir.to_string_lossy().to_string());
            args.push("-o".to_string());
            args.push(format!(
                "%(playlist_title,playlist_id).{}s/%(playlist_index&{{:02d}} - |)s%(title).{}s [%(id)s].%(ext)s",
                folder_max_chars, title_max_chars
            ));
        }
    } else {
        args.push("-P".to_string());
        args.push(output_dir.to_string_lossy().to_string());
        args.push("-o".to_string());
        args.push(format!(
            "%(title).{}s [%(id)s].%(ext)s",
            title_max_chars
        ));
    }
}

/// Inspects a URL (Video, Short, Reel, or Playlist) and returns rich `VideoMetadata`.
/// Automatically tries fallback bypass strategies if the site blocks the initial inspection.
pub fn inspect_url_blocking(url: &str, settings: &AppSettings) -> Result<VideoMetadata, String> {
    let attempts = if settings.auto_bypass_fallbacks {
        vec![
            BypassAttempt::StandardWithCookies,
            BypassAttempt::MobileClientBypass,
            BypassAttempt::EmbeddedTvFallback,
        ]
    } else {
        vec![BypassAttempt::StandardWithCookies]
    };

    let mut last_err = String::new();

    for attempt in attempts {
        match run_single_inspect(url, settings, attempt) {
            Ok(meta) => return Ok(meta),
            Err(e) => {
                last_err = e;
            }
        }
    }

    Err(last_err)
}

fn run_single_inspect(
    url: &str,
    settings: &AppSettings,
    attempt: BypassAttempt,
) -> Result<VideoMetadata, String> {
    let ytdlp = resolve_ytdlp_executable();
    let mut args = vec![
        "--dump-single-json".to_string(),
        "--flat-playlist".to_string(),
    ];
    apply_bypass_and_network_args(&mut args, url, settings, attempt);
    args.push(url.to_string());

    let mut cmd = Command::new(&ytdlp);
    cmd.args(&args);
    configure_command_platform(&mut cmd);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute yt-dlp ({:?}): {}", ytdlp, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(extract_clean_error(&stderr));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse video metadata JSON: {}", e))?;

    parse_video_metadata_json(url, &json)
}

pub fn parse_video_metadata_json(
    original_url: &str,
    json: &serde_json::Value,
) -> Result<VideoMetadata, String> {
    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled Media")
        .to_string();

    let uploader = json
        .get("uploader")
        .or_else(|| json.get("channel"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown Creator")
        .to_string();

    let duration_secs = json.get("duration").and_then(|v| {
        v.as_u64()
            .or_else(|| v.as_f64().map(|f| f.round() as u64))
    });

    let thumbnail_url = json
        .get("thumbnail")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            json.get("thumbnails")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.last())
                .and_then(|t| t.get("url"))
                .and_then(|u| u.as_str())
                .map(|s| s.to_string())
        });

    let extractor = json
        .get("extractor_key")
        .or_else(|| json.get("extractor"))
        .and_then(|v| v.as_str())
        .unwrap_or("generic")
        .to_string();

    let webpage_url = json
        .get("webpage_url")
        .and_then(|v| v.as_str())
        .unwrap_or(original_url)
        .to_string();

    let is_playlist = json.get("_type").and_then(|v| v.as_str()) == Some("playlist")
        || json
            .get("entries")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);

    let mut playlist_entries = Vec::new();
    if let Some(entries) = json.get("entries").and_then(|v| v.as_array()) {
        for (i, entry) in entries.iter().enumerate() {
            let entry_title = entry
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Video Item")
                .to_string();
            let entry_id = entry
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let entry_url = entry
                .get("url")
                .or_else(|| entry.get("webpage_url"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let dur = entry
                .get("duration")
                .and_then(|v| v.as_f64())
                .map(|d| {
                    let s = d as u64;
                    format!("{:02}:{:02}", s / 60, s % 60)
                })
                .unwrap_or_else(|| "--:--".to_string());

            playlist_entries.push(PlaylistEntry {
                index: i + 1,
                id: entry_id,
                title: entry_title,
                url: entry_url,
                duration_str: dur,
            });
        }
    }

    let mut formats = Vec::new();
    if let Some(raw_formats) = json.get("formats").and_then(|v| v.as_array()) {
        for f in raw_formats {
            let format_id = f
                .get("format_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if format_id.is_empty() {
                continue;
            }
            let ext = f
                .get("ext")
                .and_then(|v| v.as_str())
                .unwrap_or("mp4")
                .to_string();
            let vcodec = f
                .get("vcodec")
                .and_then(|v| v.as_str())
                .unwrap_or("none")
                .to_string();
            let acodec = f
                .get("acodec")
                .and_then(|v| v.as_str())
                .unwrap_or("none")
                .to_string();

            if vcodec == "none" && acodec == "none" {
                continue;
            }
            if ext == "mhtml" {
                continue;
            }

            let is_audio_only = vcodec == "none" && acodec != "none";
            let height = f.get("height").and_then(|v| v.as_u64()).map(|h| h as u32);
            let fps = f.get("fps").and_then(|v| v.as_f64()).map(|v| v as f32);
            let filesize_approx = f
                .get("filesize")
                .or_else(|| f.get("filesize_approx"))
                .and_then(|v| v.as_u64());
            let note = f
                .get("format_note")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let resolution = if is_audio_only {
                "Audio".to_string()
            } else if let Some(h) = height {
                format!("{}p", h)
            } else {
                f.get("resolution")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Video")
                    .to_string()
            };

            formats.push(FormatOption {
                format_id,
                ext,
                resolution,
                height,
                fps,
                vcodec,
                acodec,
                filesize_approx,
                note,
                is_audio_only,
            });
        }
    }

    // Sort video formats from highest resolution to lowest, followed by audio formats
    formats.sort_by(|a, b| match (a.is_audio_only, b.is_audio_only) {
        (false, true) => std::cmp::Ordering::Less,
        (true, false) => std::cmp::Ordering::Greater,
        _ => b
            .height
            .unwrap_or(0)
            .cmp(&a.height.unwrap_or(0))
            .then_with(|| {
                b.filesize_approx
                    .unwrap_or(0)
                    .cmp(&a.filesize_approx.unwrap_or(0))
            }),
    });

    Ok(VideoMetadata {
        url: original_url.to_string(),
        title,
        uploader,
        duration_secs,
        thumbnail_url,
        extractor,
        webpage_url,
        is_playlist,
        formats,
        playlist_entries,
    })
}

/// Executes a download task in a blocking worker thread, streaming real-time progress events
/// and automatically retrying with anti-block bypass strategies if the site blocks the request.
pub fn execute_download_task_blocking(
    task: DownloadTask,
    settings: AppSettings,
    sender: async_channel::Sender<DownloadProgressEvent>,
) {
    let cancel_token = register_cancel_token(task.id);
    let _ = std::fs::create_dir_all(&task.output_dir);

    let _ = sender.send_blocking(DownloadProgressEvent::Started {
        task_id: task.id,
        status_text: "Starting download...".to_string(),
    });

    let attempts = if settings.auto_bypass_fallbacks {
        vec![
            BypassAttempt::StandardWithCookies,
            BypassAttempt::MobileClientBypass,
            BypassAttempt::EmbeddedTvFallback,
        ]
    } else {
        vec![BypassAttempt::StandardWithCookies]
    };

    let mut last_error = String::new();

    for (idx, attempt) in attempts.iter().enumerate() {
        if cancel_token.load(Ordering::SeqCst) {
            remove_cancel_token(task.id);
            let _ = sender.send_blocking(DownloadProgressEvent::Cancelled { task_id: task.id });
            return;
        }

        if idx > 0 {
            let _ = sender.send_blocking(DownloadProgressEvent::BypassRetry {
                task_id: task.id,
                attempt: (idx + 1) as u8,
                strategy_name: attempt.strategy_label().to_string(),
            });
        }

        let relax_constraints = *attempt == BypassAttempt::EmbeddedTvFallback;
        match run_single_download_attempt(
            &task,
            &settings,
            *attempt,
            relax_constraints,
            &cancel_token,
            &sender,
        ) {
            Ok((resolved_title, output_path)) => {
                remove_cancel_token(task.id);
                let _ = sender.send_blocking(DownloadProgressEvent::Completed {
                    task_id: task.id,
                    title: resolved_title,
                    file_path: output_path,
                });
                return;
            }
            Err(AttemptFailure::Cancelled) => {
                remove_cancel_token(task.id);
                let _ = sender.send_blocking(DownloadProgressEvent::Cancelled { task_id: task.id });
                return;
            }
            Err(AttemptFailure::Error(err)) => {
                last_error = err;
            }
        }
    }

    remove_cancel_token(task.id);
    let _ = sender.send_blocking(DownloadProgressEvent::Failed {
        task_id: task.id,
        error: last_error,
    });
}

enum AttemptFailure {
    Cancelled,
    Error(String),
}

fn run_single_download_attempt(
    task: &DownloadTask,
    settings: &AppSettings,
    attempt: BypassAttempt,
    relax_constraints: bool,
    cancel_token: &Arc<AtomicBool>,
    sender: &async_channel::Sender<DownloadProgressEvent>,
) -> Result<(String, PathBuf), AttemptFailure> {
    let ytdlp = resolve_ytdlp_executable();
    // Pass --no-quiet so --print does not suppress [download] Downloading playlist / item lines
    let mut args = vec![
        "--newline".to_string(),
        "--progress".to_string(),
        "--no-quiet".to_string(),
    ];

    apply_bypass_and_network_args(&mut args, &task.url, settings, attempt);
    apply_format_args(
        &mut args,
        &task.preset,
        settings,
        &task.output_dir,
        &task.playlist_items,
        &task.title,
        relax_constraints,
    );

    // Print markers so we can capture playlist item metadata, exact title, and final file path reliably
    args.push("--print".to_string());
    args.push("video:__OMNI_ITEM__:%(playlist_index)s|||%(n_entries)s|||%(playlist_count)s|||%(playlist_title)s|||%(title)s".to_string());
    args.push("--print".to_string());
    args.push("before_dl:__OMNI_TITLE__:%(title)s".to_string());
    args.push("--print".to_string());
    args.push("after_move:__OMNI_FILEPATH__:%(filepath)s".to_string());
    args.push(task.url.clone());

    let mut cmd = Command::new(&ytdlp);
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_command_platform(&mut cmd);

    let mut child = cmd
        .spawn()
        .map_err(|e| AttemptFailure::Error(format!("Could not launch yt-dlp: {}", e)))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AttemptFailure::Error("Failed to capture yt-dlp stdout".to_string()))?;
    let stderr = child.stderr.take();

    // Read stderr using lossy UTF-8 byte reading so non-UTF-8 console bytes on Windows never abort stderr capture
    let stderr_handle = std::thread::spawn(move || {
        let mut err_buf = String::new();
        if let Some(err_stream) = stderr {
            let mut reader = BufReader::new(err_stream);
            let mut raw_line = Vec::with_capacity(512);
            while let Ok(bytes_read) = reader.read_until(b'\n', &mut raw_line) {
                if bytes_read == 0 {
                    break;
                }
                let line = String::from_utf8_lossy(&raw_line);
                err_buf.push_str(line.trim_end_matches(&['\r', '\n'][..]));
                err_buf.push('\n');
                raw_line.clear();
            }
        }
        err_buf
    });

    let mut reader = BufReader::new(stdout);
    let mut detected_title = task.title.clone();
    let mut detected_filepath: Option<PathBuf> = None;
    let is_audio_only = task.preset.is_audio();
    let mut stream_index: u32 = 0;
    let mut prev_raw_fraction: f64 = 0.0;
    let mut max_overall_fraction: f64 = 0.0;
    let mut playlist_title: Option<String> = if task.playlist_items.is_some() {
        Some(task.title.clone())
    } else {
        None
    };
    let mut current_item_index: u32 = 0;
    let mut total_items: u32 = task.expected_playlist_count.unwrap_or(0);
    let mut current_video_title: String = task.title.clone();
    let mut last_emit = Instant::now()
        .checked_sub(Duration::from_millis(200))
        .unwrap_or_else(Instant::now);

    let mut raw_line = Vec::with_capacity(1024);
    loop {
        if cancel_token.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AttemptFailure::Cancelled);
        }

        raw_line.clear();
        let Ok(bytes_read) = reader.read_until(b'\n', &mut raw_line) else {
            break;
        };
        if bytes_read == 0 {
            break;
        }

        let line_cow = String::from_utf8_lossy(&raw_line);
        let trimmed = line_cow.trim();
        if trimmed.is_empty() {
            continue;
        }

        // 1. Detect playlist title header from yt-dlp: `[download] Downloading playlist: <Title>`
        if let Some(pl_name) = trimmed.strip_prefix("[download] Downloading playlist:") {
            let clean_pl = pl_name.trim().to_string();
            if !clean_pl.is_empty() && clean_pl != "NA" {
                playlist_title = Some(clean_pl.clone());
                detected_title = clean_pl.clone();
                let _ = sender.send_blocking(DownloadProgressEvent::PlaylistDetected {
                    task_id: task.id,
                    playlist_title: clean_pl,
                    total_items,
                });
            }
            continue;
        }

        // 2. Detect sequential playlist item transition: `[download] Downloading item X of Y`
        if let Some((item_idx, item_total)) = parse_ytdlp_playlist_item_line(trimmed) {
            stream_index = 0;
            prev_raw_fraction = 0.0;
            max_overall_fraction = 0.0;
            current_item_index = item_idx;
            total_items = item_total;
            let _ = sender.send_blocking(DownloadProgressEvent::PlaylistItemStarted {
                task_id: task.id,
                current_index: current_item_index,
                total_items,
                video_title: format!(
                    "Fetching metadata for video {} of {}...",
                    current_item_index, total_items
                ),
                playlist_title: playlist_title.clone(),
            });
            continue;
        }

        // 3. Detect structured per-video metadata marker: `__OMNI_ITEM__:idx\tn_entries\tplaylist_count\tpl_title\tvid_title`
        if let Some(item_meta) = parse_omni_item_marker(trimmed) {
            if total_items == 0 {
                if let Some(n) = item_meta.total_items {
                    total_items = n;
                }
            }
            if current_item_index == 0 {
                if let Some(idx) = item_meta.item_index {
                    current_item_index = idx;
                    stream_index = 0;
                    prev_raw_fraction = 0.0;
                    max_overall_fraction = 0.0;
                }
            }
            if let Some(pl_t) = item_meta.playlist_title {
                playlist_title = Some(pl_t.clone());
                detected_title = pl_t;
            }
            if !item_meta.video_title.is_empty() {
                current_video_title = item_meta.video_title.clone();
                if total_items <= 1 && playlist_title.is_none() {
                    detected_title = current_video_title.clone();
                }
            }

            if total_items > 1 || playlist_title.is_some() {
                let _ = sender.send_blocking(DownloadProgressEvent::PlaylistItemStarted {
                    task_id: task.id,
                    current_index: current_item_index.max(1),
                    total_items: total_items.max(1),
                    video_title: current_video_title.clone(),
                    playlist_title: playlist_title.clone(),
                });
            }
            continue;
        }

        // 4. Detect title right before download starts
        if let Some(title) = trimmed.strip_prefix("__OMNI_TITLE__:") {
            let clean_title = title.trim().to_string();
            if !clean_title.is_empty() && clean_title != "NA" {
                current_video_title = clean_title.clone();
                if total_items > 1 || playlist_title.is_some() {
                    let _ = sender.send_blocking(DownloadProgressEvent::PlaylistItemStarted {
                        task_id: task.id,
                        current_index: current_item_index.max(1),
                        total_items: total_items.max(1),
                        video_title: clean_title,
                        playlist_title: playlist_title.clone(),
                    });
                } else {
                    detected_title = clean_title.clone();
                    let _ = sender.send_blocking(DownloadProgressEvent::TitleUpdated {
                        task_id: task.id,
                        title: clean_title,
                    });
                }
            }
            continue;
        }

        // 5. Detect completed file path for each finished item
        if let Some(fp) = trimmed.strip_prefix("__OMNI_FILEPATH__:") {
            let path = PathBuf::from(fp.trim());
            let norm_path = metadata::normalize_path(&path);
            detected_filepath = Some(norm_path.clone());
            if total_items > 1 || playlist_title.is_some() {
                let _ = sender.send_blocking(DownloadProgressEvent::PlaylistItemCompleted {
                    task_id: task.id,
                    current_index: current_item_index.max(1),
                    total_items: total_items.max(1),
                    video_title: current_video_title.clone(),
                    file_path: norm_path,
                });
            }
            continue;
        }

        if trimmed.starts_with("[download] Destination:") {
            stream_index += 1;
            prev_raw_fraction = 0.0;
        }

        if let Some(dest) = extract_destination_from_line(trimmed) {
            detected_filepath = Some(metadata::normalize_path(&dest));
        }

        if trimmed.starts_with("[Merger]")
            || trimmed.starts_with("[ExtractAudio]")
            || trimmed.starts_with("[EmbedThumbnail]")
        {
            let prefix = if total_items > 1 {
                format!("[Video {}/{}] ", current_item_index.max(1), total_items)
            } else {
                String::new()
            };
            let _ = sender.send_blocking(DownloadProgressEvent::PostProcessing {
                task_id: task.id,
                step: if is_audio_only {
                    format!(
                        "{}Converting audio and embedding cover art (ffmpeg)...",
                        prefix
                    )
                } else {
                    format!(
                        "{}Merging Video + Audio streams into MP4 (ffmpeg)...",
                        prefix
                    )
                },
            });
            continue;
        }

        if let Some(prog) = parse_ytdlp_progress_line(trimmed) {
            // Detect transition from video stream (100%) to audio stream (0%) even if Destination line was skipped
            if prog.fraction + 0.35 < prev_raw_fraction {
                stream_index = stream_index.max(1) + 1;
            }
            prev_raw_fraction = prog.fraction;

            let item_prefix = if total_items > 1 {
                format!("[Video {}/{}] ", current_item_index.max(1), total_items)
            } else {
                String::new()
            };

            let (overall_fraction, status_text) = if is_audio_only {
                let weighted = (prog.fraction * 0.94).clamp(0.0, 0.94);
                let mono = weighted.max(max_overall_fraction);
                max_overall_fraction = mono;
                (
                    mono,
                    format!(
                        "{}Downloading Audio Stream ({:.0}%)",
                        item_prefix,
                        prog.fraction * 100.0
                    ),
                )
            } else if stream_index <= 1 {
                // Stream 1: Video Track (~85% of total work)
                let weighted = (prog.fraction * 0.85).clamp(0.0, 0.85);
                let mono = weighted.max(max_overall_fraction);
                max_overall_fraction = mono;
                (
                    mono,
                    format!(
                        "{}Step 1/2: Video Track ({:.0}%) — Total {:.0}%",
                        item_prefix,
                        prog.fraction * 100.0,
                        mono * 100.0
                    ),
                )
            } else {
                // Stream 2: Audio Track (85% -> 98% of total work)
                let weighted = (0.85 + prog.fraction * 0.13).clamp(0.85, 0.98);
                let mono = weighted.max(max_overall_fraction);
                max_overall_fraction = mono;
                (
                    mono,
                    format!(
                        "{}Step 2/2: Audio Track ({:.0}%) — Total {:.0}%",
                        item_prefix,
                        prog.fraction * 100.0,
                        mono * 100.0
                    ),
                )
            };

            if last_emit.elapsed() >= Duration::from_millis(100) || prog.fraction >= 0.999 {
                last_emit = Instant::now();
                let _ = sender.send_blocking(DownloadProgressEvent::Progress {
                    task_id: task.id,
                    fraction: overall_fraction,
                    speed: prog.speed,
                    eta: prog.eta,
                    status_text,
                });
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| AttemptFailure::Error(format!("Failed waiting for yt-dlp: {}", e)))?;

    let stderr_text = stderr_handle.join().unwrap_or_default();

    if cancel_token.load(Ordering::SeqCst) {
        return Err(AttemptFailure::Cancelled);
    }

    if !status.success() {
        return Err(AttemptFailure::Error(extract_clean_error(&stderr_text)));
    }

    let final_path = detected_filepath.unwrap_or_else(|| task.output_dir.clone());
    Ok((detected_title, final_path))
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedOmniItem {
    pub item_index: Option<u32>,
    pub total_items: Option<u32>,
    pub playlist_title: Option<String>,
    pub video_title: String,
}

/// Parses `[download] Downloading item 3 of 19` or `[download] Downloading video 3 of 19`
pub fn parse_ytdlp_playlist_item_line(line: &str) -> Option<(u32, u32)> {
    let rest = line
        .strip_prefix("[download] Downloading item ")
        .or_else(|| line.strip_prefix("[download] Downloading video "))?;
    let mut parts = rest.split_whitespace();
    let idx: u32 = parts.next()?.parse().ok()?;
    let of_word = parts.next()?;
    if of_word != "of" {
        return None;
    }
    let total: u32 = parts.next()?.parse().ok()?;
    Some((idx, total))
}

/// Parses `__OMNI_ITEM__:<playlist_index>|||<n_entries>|||<playlist_count>|||<playlist_title>|||<title>`
/// (also supports `\t` separator for backwards compatibility).
pub fn parse_omni_item_marker(line: &str) -> Option<ParsedOmniItem> {
    let rest = line.strip_prefix("__OMNI_ITEM__:")?;
    let parts: Vec<&str> = if rest.contains("|||") {
        rest.splitn(5, "|||").collect()
    } else {
        rest.splitn(5, '\t').collect()
    };
    if parts.len() < 5 {
        return None;
    }
    let item_index = parts[0].trim().parse::<u32>().ok();
    let n_entries = parts[1].trim().parse::<u32>().ok();
    let playlist_count = parts[2].trim().parse::<u32>().ok();
    let total_items = n_entries.or(playlist_count);
    let pl_raw = parts[3].trim();
    let playlist_title = if !pl_raw.is_empty() && pl_raw != "NA" {
        Some(pl_raw.to_string())
    } else {
        None
    };
    let vid_raw = parts[4].trim();
    let video_title = if !vid_raw.is_empty() && vid_raw != "NA" {
        vid_raw.to_string()
    } else {
        String::new()
    };

    Some(ParsedOmniItem {
        item_index,
        total_items,
        playlist_title,
        video_title,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedProgress {
    pub fraction: f64,
    pub speed: String,
    pub eta: String,
}

/// Parses a `yt-dlp` progress line such as:
/// `[download]  45.2% of   24.50MiB at    3.12MiB/s ETA 00:04`
pub fn parse_ytdlp_progress_line(line: &str) -> Option<ParsedProgress> {
    if !line.starts_with("[download]") {
        return None;
    }

    let rest = line.strip_prefix("[download]")?.trim();
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    let first = *tokens.first()?;
    let pct_str = first.strip_suffix('%')?;
    let pct: f64 = pct_str.parse().ok()?;
    let fraction = (pct / 100.0).clamp(0.0, 1.0);

    let mut speed = String::from("-- MB/s");
    let mut eta = String::from("--:--");

    for (i, token) in tokens.iter().enumerate() {
        if *token == "at" {
            if let Some(s) = tokens.get(i + 1) {
                if *s != "Unknown" {
                    speed = (*s).to_string();
                }
            }
        } else if *token == "ETA" {
            if let Some(e) = tokens.get(i + 1) {
                if *e != "Unknown" {
                    eta = (*e).to_string();
                }
            }
        }
    }

    Some(ParsedProgress {
        fraction,
        speed,
        eta,
    })
}

fn extract_destination_from_line(line: &str) -> Option<PathBuf> {
    if let Some(rest) = line.strip_prefix("[download] Destination:") {
        return Some(PathBuf::from(rest.trim()));
    }
    if let Some(rest) = line.strip_prefix("[ExtractAudio] Destination:") {
        return Some(PathBuf::from(rest.trim()));
    }
    if let Some(rest) = line.strip_prefix("[Merger] Merging formats into \"") {
        if let Some(stripped) = rest.strip_suffix('"') {
            return Some(PathBuf::from(stripped));
        }
    }
    None
}

fn extract_clean_error(stderr: &str) -> String {
    for line in stderr.lines().rev() {
        let trimmed = line.trim();
        if let Some(err) = trimmed.strip_prefix("ERROR:") {
            return err.trim().to_string();
        }
    }
    let fallback = stderr.lines().last().unwrap_or("Download failed").trim();
    if fallback.is_empty() {
        "Site blocked request or unsupported URL. Try enabling Browser Cookies in the toolbar."
            .to_string()
    } else {
        fallback.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ytdlp_progress_line_standard() {
        let line = "[download]  45.2% of   24.50MiB at    3.12MiB/s ETA 00:04";
        let parsed = parse_ytdlp_progress_line(line).expect("Should parse valid progress line");
        assert!((parsed.fraction - 0.452).abs() < 1e-5);
        assert_eq!(parsed.speed, "3.12MiB/s");
        assert_eq!(parsed.eta, "00:04");
    }

    #[test]
    fn test_parse_ytdlp_progress_line_complete() {
        let line = "[download] 100% of   12.00MiB in 00:02 at 5.40MiB/s";
        let parsed = parse_ytdlp_progress_line(line).expect("Should parse 100% progress line");
        assert!((parsed.fraction - 1.0).abs() < 1e-5);
        assert_eq!(parsed.speed, "5.40MiB/s");
    }

    #[test]
    fn test_extract_destination() {
        let line = "[Merger] Merging formats into \"/home/user/Downloads/Video [123].mp4\"";
        let path = extract_destination_from_line(line).unwrap();
        assert_eq!(path, PathBuf::from("/home/user/Downloads/Video [123].mp4"));
    }

    #[test]
    fn test_bypass_args_youtube_mobile() {
        let settings = AppSettings::default();
        let mut args = Vec::new();
        apply_bypass_and_network_args(
            &mut args,
            "https://www.youtube.com/shorts/abcdefghijk",
            &settings,
            BypassAttempt::MobileClientBypass,
        );
        assert!(args.contains(&"--geo-bypass".to_string()));
        assert!(args.contains(&"--js-runtimes".to_string()));
        assert!(args.contains(&"node".to_string()));
        assert!(args.contains(&"youtube:player_client=ios,web,android".to_string()));
    }

    #[test]
    fn test_firefox_and_browser_cookie_args() {
        let mut settings = AppSettings::default();
        settings.cookie_source = BrowserCookieSource::Firefox;
        let mut args = Vec::new();
        apply_bypass_and_network_args(
            &mut args,
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            &settings,
            BypassAttempt::StandardWithCookies,
        );
        assert!(args.contains(&"--js-runtimes".to_string()));
        assert!(args.contains(&"node".to_string()));
        if BrowserCookieSource::Firefox.is_available_on_system() {
            assert!(args.contains(&"--cookies-from-browser".to_string()));
        }
    }

    #[test]
    fn test_parse_playlist_item_and_omni_marker() {
        assert_eq!(
            parse_ytdlp_playlist_item_line("[download] Downloading item 3 of 19"),
            Some((3, 19))
        );
        assert_eq!(
            parse_ytdlp_playlist_item_line("[download] Downloading video 1 of 5"),
            Some((1, 5))
        );

        let marker = "__OMNI_ITEM__:1|||2|||19|||Google - Year In Search (unofficial)|||2016 — Year in Search";
        let parsed = parse_omni_item_marker(marker).expect("Should parse __OMNI_ITEM__");
        assert_eq!(parsed.item_index, Some(1));
        assert_eq!(parsed.total_items, Some(2));
        assert_eq!(
            parsed.playlist_title.as_deref(),
            Some("Google - Year In Search (unofficial)")
        );
        assert_eq!(parsed.video_title, "2016 — Year in Search");
    }

    #[test]
    fn test_playlist_dedicated_folder_and_length_args() {
        let settings = AppSettings::default();
        let base_dir = std::env::temp_dir().join("omni_test_dl");
        let mut args = Vec::new();
        apply_format_args(
            &mut args,
            &QualityPreset::Video1080p,
            &settings,
            &base_dir,
            &Some("1-5".to_string()),
            "دورة تعلم Rust: من الصفر | 2026",
            false,
        );
        assert!(args.contains(&"--yes-playlist".to_string()));
        assert!(args.contains(&"--windows-filenames".to_string()));
        let p_idx = args.iter().position(|a| a == "-P").expect("Should have -P");
        let target_folder = &args[p_idx + 1];
        assert!(
            target_folder.contains("دورة تعلم Rust - من الصفر - 2026"),
            "Playlist should be saved in its own sanitized subfolder: {}",
            target_folder
        );
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_managed_deno_toggle_priority() {
        let mut settings = AppSettings::default();
        assert!(settings.use_managed_deno, "Managed deno should be enabled by default");

        settings.use_managed_deno = false;
        let mut args = Vec::new();
        apply_bypass_and_network_args(
            &mut args,
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            &settings,
            BypassAttempt::StandardWithCookies,
        );
        // When use_managed_deno is false, it should pass bare "deno" (system PATH priority), not "deno:/path/..."
        assert!(args.contains(&"deno".to_string()));
        assert!(!args.iter().any(|a| a.starts_with("deno:")));
    }
}
