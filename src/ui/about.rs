use crate::metadata;
use crate::models::InstalledToolVersions;
use adw::prelude::*;

pub fn show_about_dialog(parent: &impl IsA<gtk::Widget>) {
    let versions = InstalledToolVersions::load();
    let ytdlp_ver = if versions.ytdlp_version.is_empty() {
        "Not installed / Checking".to_string()
    } else {
        versions.ytdlp_version
    };
    let ffmpeg_ver = if versions.ffmpeg_version.is_empty() {
        "System / Checking".to_string()
    } else {
        versions.ffmpeg_version
    };

    let deno_ver = if versions.deno_version.is_empty() {
        "System / Checking".to_string()
    } else {
        versions.deno_version
    };

    let debug_info = format!(
        "Application: {} v{}\n\
         App ID: {}\n\
         OS: {} ({})\n\
         Managed Binaries Path: {}\n\
         yt-dlp Version: {}\n\
         ffmpeg Version: {}\n\
         deno Version: {}\n\
         HTTP Backend: libsoup-3.0 (soup3)",
        metadata::APP_NAME,
        metadata::APP_VERSION,
        metadata::APP_ID,
        std::env::consts::OS,
        std::env::consts::ARCH,
        metadata::managed_bin_dir().to_string_lossy(),
        ytdlp_ver,
        ffmpeg_ver,
        deno_ver,
    );

    let about = adw::AboutDialog::builder()
        .application_name(metadata::APP_NAME)
        .application_icon(metadata::ICON_NAME)
        .version(metadata::APP_VERSION)
        .developer_name(metadata::APP_DEVELOPER)
        .developers(vec![metadata::APP_DEVELOPER])
        .copyright("© 2026 Yucef Sourani")
        .license_type(gtk::License::Gpl30)
        .website(metadata::APP_WEBSITE)
        .issue_url(metadata::APP_ISSUE_URL)
        .comments(
            "Modern cross-platform Video, Reels, Shorts, Playlist and MP3 Downloader \
             with automatic anti-block bypass and self-managed yt-dlp, ffmpeg and deno updates via libsoup3.",
        )
        .debug_info(&debug_info)
        .debug_info_filename("arvdownloader-diagnostics.txt")
        .build();

    about.add_acknowledgement_section(
        Some("Core Media and Network Engines"),
        &[
            "yt-dlp https://github.com/yt-dlp/yt-dlp",
            "FFmpeg https://ffmpeg.org",
            "Deno https://deno.com",
            "GTK4 and Libadwaita https://gtk.org",
            "libsoup3 https://libsoup.org",
        ],
    );

    about.present(Some(parent));
}
