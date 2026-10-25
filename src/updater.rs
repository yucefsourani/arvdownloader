use crate::metadata;
use crate::models::{AppSettings, InstalledToolVersions, ToolUpdateEvent};
use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib;
use soup::prelude::*;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

fn is_status_success(status: soup::Status) -> bool {
    matches!(
        status,
        soup::Status::Ok
            | soup::Status::Created
            | soup::Status::Accepted
            | soup::Status::NonAuthoritative
            | soup::Status::NoContent
            | soup::Status::PartialContent
    )
}

#[derive(Debug, Clone)]
struct ReleaseAssetInfo {
    tag_name: String,
    download_url: String,
    asset_name: String,
    size_bytes: u64,
}

/// Fetches raw bytes (such as video thumbnails) asynchronously using `libsoup3`
pub async fn fetch_bytes_with_soup(url: &str) -> Result<glib::Bytes, String> {
    let session = soup::Session::new();
    session.set_user_agent(metadata::HTTP_USER_AGENT);
    session.set_timeout(30);

    let msg = soup::Message::new("GET", url).map_err(|e| format!("Invalid URL: {}", e))?;
    let bytes = session
        .send_and_read_future(&msg, glib::Priority::DEFAULT)
        .await
        .map_err(|e| format!("Network error via libsoup3: {}", e))?;

    let status = msg.status();
    if !is_status_success(status) {
        return Err(format!("HTTP status {:?} when fetching {}", status, url));
    }

    Ok(bytes)
}

/// Runs the startup & manual update workflow for `yt-dlp`, `ffmpeg`, and `deno` using `libsoup3`.
///
/// 1. Ensures `~/.video_downloader/bin` exists.
/// 2. On first launch, if bundled binaries exist next to the executable (`bundled_bin/`), copies them
///    into `~/.video_downloader/bin` immediately.
/// 3. Queries GitHub Releases via `libsoup3` for `yt-dlp`, `ffmpeg`, and `deno`.
/// 4. Downloads any missing or newer versions into `~/.video_downloader/staging` with live progress bar updates,
///    extracts archives (`zip` on Windows, `tar.xz` on Linux), and moves binaries to `~/.video_downloader/bin`.
pub async fn run_tools_updater(
    sender: async_channel::Sender<ToolUpdateEvent>,
    force_reinstall: bool,
) {
    let _ = sender
        .send(ToolUpdateEvent::Started {
            message: "Initializing ~/.video_downloader/bin and checking yt-dlp, ffmpeg, and deno...".to_string(),
        })
        .await;

    // Step 1: Seed from bundled directory on first launch if managed binaries are missing
    if let Err(e) = seed_from_bundled_if_needed(&sender).await {
        let _ = sender
            .send(ToolUpdateEvent::Warning {
                message: format!("Bundled seed notice: {}", e),
            })
            .await;
    }

    let mut installed = InstalledToolVersions::load();
    let ytdlp_path = metadata::managed_ytdlp_path();
    let ffmpeg_path = metadata::managed_ffmpeg_path();
    let deno_path = metadata::managed_deno_path();

    // Detect local yt-dlp version if binary exists but versions.json was empty
    if installed.ytdlp_version.is_empty() && ytdlp_path.exists() {
        if let Some(ver) = query_local_ytdlp_version(&ytdlp_path).await {
            installed.ytdlp_version = ver;
            installed.save();
        }
    }

    // Detect local deno version if binary exists but versions.json was empty
    if installed.deno_version.is_empty() && deno_path.exists() {
        if let Some(ver) = query_local_deno_version(&deno_path).await {
            installed.deno_version = ver;
            installed.save();
        }
    }

    let session = soup::Session::new();
    session.set_user_agent(metadata::HTTP_USER_AGENT);
    session.set_timeout(60);

    let mut updated_any = false;

    // Step 2: Check and update yt-dlp via libsoup3
    let _ = sender
        .send(ToolUpdateEvent::Progress {
            tool_name: "yt-dlp".to_string(),
            fraction: 0.05,
            status_text: "Checking GitHub for yt-dlp updates via libsoup3...".to_string(),
        })
        .await;

    let wanted_ytdlp_assets: &[&str] = if cfg!(windows) {
        &["yt-dlp.exe"]
    } else {
        &["yt-dlp_linux", "yt-dlp"]
    };

    match fetch_latest_github_release(&session, metadata::YTDLP_GITHUB_API, wanted_ytdlp_assets).await
    {
        Ok(release) => {
            let needs_ytdlp = force_reinstall
                || !ytdlp_path.exists()
                || installed.ytdlp_version != release.tag_name;

            if needs_ytdlp {
                let staging_file = metadata::staging_dir().join(&release.asset_name);
                match download_file_with_soup_progress(
                    &session,
                    &release.download_url,
                    &staging_file,
                    release.size_bytes,
                    "yt-dlp",
                    &sender,
                )
                .await
                {
                    Ok(()) => {
                        let target = ytdlp_path.clone();
                        let move_res = gio::spawn_blocking(move || -> Result<(), String> {
                            install_binary_atomically(&staging_file, &target)
                        })
                        .await
                        .unwrap_or_else(|_| Err("Task panicked during yt-dlp install".to_string()));

                        match move_res {
                            Ok(()) => {
                                installed.ytdlp_version = release.tag_name;
                                installed.save();
                                updated_any = true;
                            }
                            Err(e) => {
                                let _ = sender
                                    .send(ToolUpdateEvent::Warning {
                                        message: format!("Failed to install yt-dlp: {}", e),
                                    })
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = sender
                            .send(ToolUpdateEvent::Warning {
                                message: format!("Could not download yt-dlp update: {}", e),
                            })
                            .await;
                    }
                }
            }
        }
        Err(e) => {
            let _ = sender
                .send(ToolUpdateEvent::Warning {
                    message: format!("yt-dlp update check skipped/offline: {}", e),
                })
                .await;
        }
    }

    // Step 3: Check and update ffmpeg & ffprobe via libsoup3
    let _ = sender
        .send(ToolUpdateEvent::Progress {
            tool_name: "ffmpeg".to_string(),
            fraction: 0.50,
            status_text: "Checking GitHub for ffmpeg updates via libsoup3...".to_string(),
        })
        .await;

    let wanted_ffmpeg_assets: &[&str] = if cfg!(windows) {
        &[
            "ffmpeg-master-latest-win64-gpl.zip",
            "ffmpeg-n7.1-latest-win64-gpl-7.1.zip",
            "win64-gpl.zip",
        ]
    } else {
        &[
            "ffmpeg-master-latest-linux64-gpl.tar.xz",
            "ffmpeg-n7.1-latest-linux64-gpl-7.1.tar.xz",
            "linux64-gpl.tar.xz",
        ]
    };

    // If on Linux and system ffmpeg is available while user hasn't downloaded managed ffmpeg yet,
    // we still download/manage it in ~/.video_downloader/bin as requested, with fallback to system if offline.
    match fetch_latest_github_release(&session, metadata::FFMPEG_GITHUB_API, wanted_ffmpeg_assets)
        .await
    {
        Ok(release) => {
            let needs_ffmpeg = force_reinstall
                || !ffmpeg_path.exists()
                || installed.ffmpeg_version != release.tag_name;

            if needs_ffmpeg {
                let staging_archive = metadata::staging_dir().join(&release.asset_name);
                match download_file_with_soup_progress(
                    &session,
                    &release.download_url,
                    &staging_archive,
                    release.size_bytes,
                    "ffmpeg",
                    &sender,
                )
                .await
                {
                    Ok(()) => {
                        let _ = sender
                            .send(ToolUpdateEvent::Progress {
                                tool_name: "ffmpeg".to_string(),
                                fraction: 0.92,
                                status_text:
                                    "Extracting ffmpeg and ffprobe and moving to ~/.video_downloader/bin..."
                                        .to_string(),
                            })
                            .await;

                        let bin_dir = metadata::managed_bin_dir();
                        let extract_res = gio::spawn_blocking(move || -> Result<(), String> {
                            extract_and_install_ffmpeg(&staging_archive, &bin_dir)
                        })
                        .await
                        .unwrap_or_else(|_| {
                            Err("Task panicked during ffmpeg extraction".to_string())
                        });

                        match extract_res {
                            Ok(()) => {
                                installed.ffmpeg_version = release.tag_name;
                                installed.save();
                                updated_any = true;
                            }
                            Err(e) => {
                                let _ = sender
                                    .send(ToolUpdateEvent::Warning {
                                        message: format!("Failed to extract ffmpeg: {}", e),
                                    })
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = sender
                            .send(ToolUpdateEvent::Warning {
                                message: format!("Could not download ffmpeg update: {}", e),
                            })
                            .await;
                    }
                }
            }
        }
        Err(e) => {
            let _ = sender
                .send(ToolUpdateEvent::Warning {
                    message: format!("ffmpeg update check skipped/offline: {}", e),
                })
                .await;
        }
    }

    // Step 4: Check and update `deno` in ~/.video_downloader/bin via libsoup3
    // - If `use_managed_deno` is enabled (default = true): check GitHub Releases for deno and download/update in `~/.video_downloader/bin`.
    // - If `use_managed_deno` is disabled: prioritize system PATH binaries, only downloading deno if no system JS runtime exists.
    let settings = AppSettings::load();
    let should_manage_deno = settings.use_managed_deno || !has_any_js_runtime().await;

    if should_manage_deno {
        let _ = sender
            .send(ToolUpdateEvent::Progress {
                tool_name: "deno".to_string(),
                fraction: 0.72,
                status_text: "Checking GitHub for deno updates via libsoup3...".to_string(),
            })
            .await;

        let wanted_deno_assets: &[&str] = if cfg!(windows) {
            &["deno-x86_64-pc-windows-msvc.zip"]
        } else {
            &["deno-x86_64-unknown-linux-gnu.zip"]
        };

        match fetch_latest_github_release(&session, metadata::DENO_GITHUB_API, wanted_deno_assets)
            .await
        {
            Ok(release) => {
                let needs_deno = force_reinstall
                    || !deno_path.exists()
                    || installed.deno_version != release.tag_name;

                if needs_deno {
                    let staging_zip = metadata::staging_dir().join(&release.asset_name);
                    match download_file_with_soup_progress(
                        &session,
                        &release.download_url,
                        &staging_zip,
                        release.size_bytes,
                        "deno",
                        &sender,
                    )
                    .await
                    {
                        Ok(()) => {
                            let _ = sender
                                .send(ToolUpdateEvent::Progress {
                                    tool_name: "deno".to_string(),
                                    fraction: 0.96,
                                    status_text:
                                        "Extracting deno and moving to ~/.video_downloader/bin..."
                                            .to_string(),
                                })
                                .await;

                            let bin_dir = metadata::managed_bin_dir();
                            let extract_res = gio::spawn_blocking(move || -> Result<(), String> {
                                extract_and_install_deno(&staging_zip, &bin_dir)
                            })
                            .await
                            .unwrap_or_else(|_| {
                                Err("Task panicked during deno extraction".to_string())
                            });

                            match extract_res {
                                Ok(()) => {
                                    installed.deno_version = release.tag_name;
                                    installed.save();
                                    updated_any = true;
                                }
                                Err(e) => {
                                    let _ = sender
                                        .send(ToolUpdateEvent::Warning {
                                            message: format!("Failed to extract deno: {}", e),
                                        })
                                        .await;
                                }
                            }
                        }
                        Err(e) => {
                            let _ = sender
                                .send(ToolUpdateEvent::Warning {
                                    message: format!("Could not download deno update: {}", e),
                                })
                                .await;
                        }
                    }
                }
            }
            Err(e) => {
                let _ = sender
                    .send(ToolUpdateEvent::Warning {
                        message: format!("deno update check skipped/offline: {}", e),
                    })
                    .await;
            }
        }
    }

    // Step 5: Final verification
    let ytdlp_ok = metadata::managed_ytdlp_path().exists();
    let ffmpeg_ok = metadata::managed_ffmpeg_path().exists();
    let deno_ok = metadata::managed_deno_path().exists();

    if !ytdlp_ok {
        let _ = sender
            .send(ToolUpdateEvent::Error {
                message: "yt-dlp binary is missing in ~/.video_downloader/bin. Please check your internet connection and click Update Tools.".to_string(),
            })
            .await;
        return;
    }

    let ytdlp_ver = if installed.ytdlp_version.is_empty() {
        "Installed".to_string()
    } else {
        installed.ytdlp_version.clone()
    };
    let ffmpeg_ver = if !installed.ffmpeg_version.is_empty() {
        installed.ffmpeg_version.clone()
    } else if ffmpeg_ok {
        "Installed".to_string()
    } else {
        "System Fallback".to_string()
    };
    let deno_ver = if !settings.use_managed_deno && !deno_ok {
        "System PATH".to_string()
    } else if !installed.deno_version.is_empty() {
        installed.deno_version.clone()
    } else if deno_ok {
        "Installed".to_string()
    } else {
        "System Fallback".to_string()
    };

    let _ = sender
        .send(ToolUpdateEvent::Completed {
            ytdlp_version: ytdlp_ver,
            ffmpeg_version: ffmpeg_ver,
            deno_version: deno_ver,
            updated_any,
        })
        .await;
}

/// On first launch (especially on Windows where `yt-dlp.exe` and `ffmpeg.exe` may be bundled in `bundled_bin/`),
/// copies them into `~/.video_downloader/bin` before checking for online updates.
async fn seed_from_bundled_if_needed(
    sender: &async_channel::Sender<ToolUpdateEvent>,
) -> Result<(), String> {
    let Some(bundled_dir) = metadata::bundled_bin_dir() else {
        return Ok(());
    };

    let tools = [
        (metadata::ytdlp_bin_name(), metadata::managed_ytdlp_path()),
        (metadata::ffmpeg_bin_name(), metadata::managed_ffmpeg_path()),
        (
            metadata::ffprobe_bin_name(),
            metadata::managed_ffprobe_path(),
        ),
        (metadata::deno_bin_name(), metadata::managed_deno_path()),
    ];

    let use_managed_deno = AppSettings::load().use_managed_deno;
    for (name, dest_path) in tools {
        if name == metadata::deno_bin_name() && !use_managed_deno {
            continue;
        }
        if !dest_path.exists() {
            let src_path = bundled_dir.join(name);
            if src_path.exists() && src_path != dest_path {
                let _ = sender
                    .send(ToolUpdateEvent::Progress {
                        tool_name: name.to_string(),
                        fraction: 0.02,
                        status_text: format!(
                            "First run: copying bundled {} to ~/.video_downloader/bin...",
                            name
                        ),
                    })
                    .await;

                let src_clone = src_path.clone();
                let dest_clone = dest_path.clone();
                gio::spawn_blocking(move || -> Result<(), String> {
                    std::fs::copy(&src_clone, &dest_clone)
                        .map_err(|e| format!("Failed copying {:?}: {}", src_clone, e))?;
                    set_executable_permissions(&dest_clone)?;
                    Ok(())
                })
                .await
                .map_err(|_| "Seed copy task failed".to_string())??;
            }
        }
    }
    Ok(())
}

/// Queries GitHub Releases API using `soup3::Session` and finds the matching platform binary asset.
async fn fetch_latest_github_release(
    session: &soup::Session,
    api_url: &str,
    candidate_names: &[&str],
) -> Result<ReleaseAssetInfo, String> {
    let msg = soup::Message::new("GET", api_url)
        .map_err(|e| format!("Invalid GitHub API URL {}: {}", api_url, e))?;

    if let Some(headers) = msg.request_headers() {
        headers.append("Accept", "application/vnd.github+json");
        headers.append("X-GitHub-Api-Version", "2022-11-28");
    }

    let bytes = session
        .send_and_read_future(&msg, glib::Priority::DEFAULT)
        .await
        .map_err(|e| format!("libsoup3 request failed: {}", e))?;

    let status = msg.status();
    if !is_status_success(status) {
        return Err(format!("GitHub API returned HTTP {:?}", status));
    }

    let json: serde_json::Value = serde_json::from_slice(bytes.as_ref())
        .map_err(|e| format!("Invalid GitHub release JSON: {}", e))?;

    let tag_name = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("latest")
        .to_string();

    let assets = json
        .get("assets")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "No assets array in GitHub release".to_string())?;

    for wanted in candidate_names {
        for asset in assets {
            let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name == *wanted || name.ends_with(wanted) {
                if let Some(url) = asset.get("browser_download_url").and_then(|v| v.as_str()) {
                    let size = asset.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                    return Ok(ReleaseAssetInfo {
                        tag_name,
                        download_url: url.to_string(),
                        asset_name: name.to_string(),
                        size_bytes: size,
                    });
                }
            }
        }
    }

    Err(format!(
        "No matching release asset found for {:?}",
        candidate_names
    ))
}

/// Streams a binary download in chunks using `soup3::Session::send_future` and `InputStreamExt::read_bytes_future`,
/// updating the GTK ProgressBar at most once every 100ms.
async fn download_file_with_soup_progress(
    session: &soup::Session,
    url: &str,
    dest_file: &Path,
    fallback_size: u64,
    tool_name: &str,
    sender: &async_channel::Sender<ToolUpdateEvent>,
) -> Result<(), String> {
    if let Some(parent) = dest_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let msg =
        soup::Message::new("GET", url).map_err(|e| format!("Invalid download URL: {}", e))?;

    let stream = session
        .send_future(&msg, glib::Priority::DEFAULT)
        .await
        .map_err(|e| format!("Failed to start libsoup3 stream: {}", e))?;

    let status = msg.status();
    if !is_status_success(status) {
        return Err(format!("Download returned HTTP {:?}", status));
    }

    let header_len = msg
        .response_headers()
        .map(|h| h.content_length())
        .unwrap_or(0);
    let total_bytes = if header_len > 0 {
        header_len as u64
    } else {
        fallback_size
    };

    let mut file =
        File::create(dest_file).map_err(|e| format!("Cannot create staging file: {}", e))?;

    let mut downloaded: u64 = 0;
    let mut last_emit = Instant::now()
        .checked_sub(Duration::from_millis(200))
        .unwrap_or_else(Instant::now);

    loop {
        let chunk = stream
            .read_bytes_future(65536, glib::Priority::DEFAULT)
            .await
            .map_err(|e| format!("Stream read error: {}", e))?;

        if chunk.is_empty() {
            break;
        }

        let slice = chunk.as_ref();
        file.write_all(slice)
            .map_err(|e| format!("Disk write error: {}", e))?;
        downloaded += slice.len() as u64;

        if last_emit.elapsed() >= Duration::from_millis(100) {
            last_emit = Instant::now();
            let fraction = if total_bytes > 0 {
                (downloaded as f64 / total_bytes as f64).clamp(0.0, 1.0)
            } else {
                0.5
            };
            let dl_mb = downloaded as f64 / (1024.0 * 1024.0);
            let status_text = if total_bytes > 0 {
                let tot_mb = total_bytes as f64 / (1024.0 * 1024.0);
                format!(
                    "Downloading {} ({:.0}% — {:.1} / {:.1} MB)...",
                    tool_name,
                    fraction * 100.0,
                    dl_mb,
                    tot_mb
                )
            } else {
                format!("Downloading {} ({:.1} MB)...", tool_name, dl_mb)
            };

            let _ = sender
                .send(ToolUpdateEvent::Progress {
                    tool_name: tool_name.to_string(),
                    fraction,
                    status_text,
                })
                .await;
        }
    }

    file.flush()
        .map_err(|e| format!("Failed flushing file: {}", e))?;
    Ok(())
}

/// Atomically installs a downloaded single binary into `~/.video_downloader/bin` and sets `+x` permissions.
fn install_binary_atomically(staging_file: &Path, target_path: &Path) -> Result<(), String> {
    if let Some(parent) = target_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    set_executable_permissions(staging_file)?;

    // On Windows, if target exists, remove it first before rename/copy
    if target_path.exists() {
        let _ = std::fs::remove_file(target_path);
    }

    if std::fs::rename(staging_file, target_path).is_err() {
        std::fs::copy(staging_file, target_path)
            .map_err(|e| format!("Failed moving binary to {:?}: {}", target_path, e))?;
        let _ = std::fs::remove_file(staging_file);
    }

    set_executable_permissions(target_path)?;
    Ok(())
}

/// Extracts `ffmpeg` and `ffprobe` from `.zip` (Windows) or `.tar.xz` (Linux) and moves them into `~/.video_downloader/bin`.
fn extract_and_install_ffmpeg(archive_path: &Path, bin_dir: &Path) -> Result<(), String> {
    let name_lossy = archive_path.to_string_lossy().to_lowercase();
    let ffmpeg_name = metadata::ffmpeg_bin_name();
    let ffprobe_name = metadata::ffprobe_bin_name();

    if name_lossy.ends_with(".zip") {
        let file =
            File::open(archive_path).map_err(|e| format!("Cannot open zip archive: {}", e))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("Invalid zip archive: {}", e))?;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("Zip entry error: {}", e))?;
            if entry.is_dir() {
                continue;
            }
            let entry_name = entry.name().replace('\\', "/");
            let file_basename = entry_name.rsplit('/').next().unwrap_or("");
            if file_basename.eq_ignore_ascii_case(ffmpeg_name)
                || file_basename.eq_ignore_ascii_case(ffprobe_name)
            {
                let temp_out = metadata::staging_dir().join(file_basename);
                let mut out_file = File::create(&temp_out)
                    .map_err(|e| format!("Cannot create extracted file: {}", e))?;
                std::io::copy(&mut entry, &mut out_file)
                    .map_err(|e| format!("Extraction write failed: {}", e))?;
                drop(out_file);

                let final_dest = bin_dir.join(file_basename);
                install_binary_atomically(&temp_out, &final_dest)?;
            }
        }
    } else if name_lossy.ends_with(".tar.xz") {
        let file =
            File::open(archive_path).map_err(|e| format!("Cannot open tar.xz archive: {}", e))?;
        let decompressor = xz2::read::XzDecoder::new(file);
        let mut archive = tar::Archive::new(decompressor);

        let entries = archive
            .entries()
            .map_err(|e| format!("Tar entries error: {}", e))?;

        for entry_res in entries {
            let mut entry = entry_res.map_err(|e| format!("Tar entry read error: {}", e))?;
            let path_in_tar = entry
                .path()
                .map_err(|e| format!("Tar path error: {}", e))?
                .to_path_buf();
            let Some(file_basename) = path_in_tar.file_name().and_then(|s| s.to_str()) else {
                continue;
            };

            if file_basename == ffmpeg_name || file_basename == ffprobe_name {
                let temp_out = metadata::staging_dir().join(file_basename);
                let mut out_file = File::create(&temp_out)
                    .map_err(|e| format!("Cannot create extracted file: {}", e))?;
                let mut buf = Vec::new();
                entry
                    .read_to_end(&mut buf)
                    .map_err(|e| format!("Tar stream read error: {}", e))?;
                out_file
                    .write_all(&buf)
                    .map_err(|e| format!("Tar write error: {}", e))?;
                drop(out_file);

                let final_dest = bin_dir.join(file_basename);
                install_binary_atomically(&temp_out, &final_dest)?;
            }
        }
    } else {
        return Err(format!("Unsupported archive format: {:?}", archive_path));
    }

    let _ = std::fs::remove_file(archive_path);
    Ok(())
}

fn set_executable_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)
            .map_err(|e| format!("Cannot read metadata for {:?}: {}", path, e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms)
            .map_err(|e| format!("Cannot chmod +x {:?}: {}", path, e))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

async fn query_local_ytdlp_version(ytdlp_path: &Path) -> Option<String> {
    let path = ytdlp_path.to_path_buf();
    gio::spawn_blocking(move || {
        let mut cmd = std::process::Command::new(&path);
        cmd.arg("--version");
        crate::downloader::configure_command_platform(&mut cmd);
        let output = cmd.output().ok()?;
        if output.status.success() {
            let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !ver.is_empty() {
                return Some(ver);
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
}

async fn query_local_deno_version(deno_path: &Path) -> Option<String> {
    let path = deno_path.to_path_buf();
    gio::spawn_blocking(move || {
        let mut cmd = std::process::Command::new(&path);
        cmd.arg("--version");
        crate::downloader::configure_command_platform(&mut cmd);
        let output = cmd.output().ok()?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("deno ") {
                    let ver_token = rest.split_whitespace().next().unwrap_or("").trim();
                    if !ver_token.is_empty() {
                        if ver_token.starts_with('v') {
                            return Some(ver_token.to_string());
                        } else {
                            return Some(format!("v{}", ver_token));
                        }
                    }
                }
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
}

/// Checks if `deno` is in `~/.video_downloader/bin` or if any supported JS runtime (`deno`, `node`, `qjs`, `bun`) is in PATH.
async fn has_any_js_runtime() -> bool {
    if metadata::managed_deno_path().exists() {
        return true;
    }
    gio::spawn_blocking(|| {
        for bin in ["deno", "node", "qjs", "quickjs", "bun"] {
            let mut cmd = std::process::Command::new(bin);
            cmd.arg("--version");
            crate::downloader::configure_command_platform(&mut cmd);
            if let Ok(out) = cmd.output() {
                if out.status.success() {
                    return true;
                }
            }
        }
        false
    })
    .await
    .unwrap_or(false)
}

fn extract_and_install_deno(archive_path: &Path, bin_dir: &Path) -> Result<(), String> {
    let deno_name = metadata::deno_bin_name();
    let file = File::open(archive_path).map_err(|e| format!("Cannot open deno zip: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Invalid deno zip: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Zip entry error: {}", e))?;
        if entry.is_dir() {
            continue;
        }
        let entry_name = entry.name().replace('\\', "/");
        let file_basename = entry_name.rsplit('/').next().unwrap_or("");
        if file_basename.eq_ignore_ascii_case(deno_name) {
            let temp_out = metadata::staging_dir().join(file_basename);
            let mut out_file = File::create(&temp_out)
                .map_err(|e| format!("Cannot create extracted deno: {}", e))?;
            std::io::copy(&mut entry, &mut out_file)
                .map_err(|e| format!("Deno extraction write failed: {}", e))?;
            drop(out_file);

            let final_dest = bin_dir.join(file_basename);
            install_binary_atomically(&temp_out, &final_dest)?;
        }
    }
    let _ = std::fs::remove_file(archive_path);
    Ok(())
}
