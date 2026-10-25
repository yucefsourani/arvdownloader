use crate::metadata;
use crate::models::{AppSettings, BrowserCookieSource, InstalledToolVersions};
use adw::prelude::*;
use gtk::gio;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Builds a `gtk::SignalListItemFactory` for the popup list of browser cookies.
/// - Shows the full text without ellipsizing or scrolling.
/// - Marks unavailable browsers `(N/A)` as non-selectable, non-activatable, and insensitive (disabled).
pub fn build_cookie_list_factory(detailed: bool) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();

    factory.connect_setup(|_, obj| {
        let Some(list_item) = obj.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let label = gtk::Label::new(None);
        label.set_halign(gtk::Align::Start);
        label.set_xalign(0.0);
        label.set_single_line_mode(true);
        label.set_margin_top(4);
        label.set_margin_bottom(4);
        label.set_margin_start(6);
        label.set_margin_end(6);
        list_item.set_child(Some(&label));
    });

    factory.connect_bind(move |_, obj| {
        let Some(list_item) = obj.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let pos = list_item.position();
        let src = BrowserCookieSource::from_index(pos);
        let available = src.is_available_on_system();

        // Disable selection & activation for unavailable browsers (N/A)
        list_item.set_selectable(available);
        list_item.set_activatable(available);

        if let Some(label) = list_item.child().and_downcast::<gtk::Label>() {
            let text = if detailed {
                src.detailed_settings_label()
            } else {
                src.short_toolbar_label()
            };
            label.set_text(&text);
            label.set_sensitive(available);
            if available {
                label.remove_css_class("dim-label");
            } else {
                label.add_css_class("dim-label");
            }
        }
    });

    factory
}

pub fn show_preferences_dialog<F, U>(
    parent: &adw::ApplicationWindow,
    settings_rc: Rc<RefCell<AppSettings>>,
    on_settings_changed: F,
    on_trigger_tools_updater: U,
) where
    F: Fn() + 'static,
    U: Fn(bool) + 'static,
{
    let dialog = adw::PreferencesDialog::builder()
        .title("Preferences")
        .content_width(600)
        .content_height(560)
        .build();

    let on_changed_rc = Rc::new(on_settings_changed);
    let on_trigger_updater_rc = Rc::new(on_trigger_tools_updater);

    let page = adw::PreferencesPage::builder()
        .title("General and Bypass")
        .icon_name("emblem-system-symbolic")
        .build();

    // 0. Appearance Group (Dark Theme enabled by default, persisted in config.json)
    let appearance_group = adw::PreferencesGroup::builder()
        .title("Appearance")
        .description("Application color scheme and theme preference")
        .build();

    let dark_theme_switch = adw::SwitchRow::builder()
        .title("Dark Theme")
        .subtitle("Use dark color scheme by default (saved and restored on startup)")
        .use_markup(false)
        .active(settings_rc.borrow().dark_theme)
        .build();

    let settings_for_theme = settings_rc.clone();
    dark_theme_switch.connect_active_notify(move |row| {
        let is_dark = row.is_active();
        crate::ui::window::apply_theme(is_dark);
        let mut cfg = settings_for_theme.borrow_mut();
        cfg.dark_theme = is_dark;
        cfg.save();
    });
    appearance_group.add(&dark_theme_switch);
    page.add(&appearance_group);

    // 1. Download Destination Group
    let dir_group = adw::PreferencesGroup::builder()
        .title("Download Location")
        .description("Where downloaded videos, Reels, Shorts, and MP3 files are saved")
        .build();

    let current_dir_str = settings_rc
        .borrow()
        .download_dir
        .to_string_lossy()
        .to_string();

    let dir_row = adw::ActionRow::builder()
        .title("Output Folder")
        .subtitle(&current_dir_str)
        .use_markup(false)
        .title_lines(1)
        .subtitle_lines(2)
        .build();

    let open_dir_btn = gtk::Button::builder()
        .icon_name("folder-open-symbolic")
        .tooltip_text("Open Output Folder")
        .valign(gtk::Align::Center)
        .build();

    let settings_for_open_dir = settings_rc.clone();
    open_dir_btn.connect_clicked(move |_| {
        let dir = settings_for_open_dir.borrow().download_dir.clone();
        let _ = std::fs::create_dir_all(&dir);
        let _ = open::that(&dir);
    });

    let choose_dir_btn = gtk::Button::builder()
        .icon_name("document-edit-symbolic")
        .label("Change")
        .tooltip_text("Change Output Folder")
        .valign(gtk::Align::Center)
        .build();

    let parent_weak = parent.downgrade();
    let settings_for_dir = settings_rc.clone();
    let dir_row_clone = dir_row.clone();
    let on_changed_dir = on_changed_rc.clone();

    choose_dir_btn.connect_clicked(move |_| {
        let Some(win) = parent_weak.upgrade() else {
            return;
        };
        let current_dir = settings_for_dir.borrow().download_dir.clone();
        let _ = std::fs::create_dir_all(&current_dir);
        let initial_folder = gio::File::for_path(&current_dir);

        let file_dialog = gtk::FileDialog::builder()
            .title("Select Download Folder")
            .initial_folder(&initial_folder)
            .modal(true)
            .build();

        let settings_inner = settings_for_dir.clone();
        let row_inner = dir_row_clone.clone();
        let cb_inner = on_changed_dir.clone();

        file_dialog.select_folder(Some(&win), gio::Cancellable::NONE, move |res| {
            if let Ok(folder) = res {
                if let Some(path) = folder.path() {
                    let norm = metadata::normalize_path(&path);
                    let _ = std::fs::create_dir_all(&norm);
                    row_inner.set_subtitle(&norm.to_string_lossy());
                    {
                        let mut cfg = settings_inner.borrow_mut();
                        cfg.download_dir = norm;
                        cfg.save();
                    }
                    cb_inner();
                }
            }
        });
    });

    dir_row.add_suffix(&open_dir_btn);
    dir_row.add_suffix(&choose_dir_btn);
    dir_group.add(&dir_row);
    page.add(&dir_group);

    // 2. Site Anti-Block & Authentication Group
    let bypass_group = adw::PreferencesGroup::builder()
        .title("Site Anti-Block and Cookies")
        .description("Resolve YouTube, Facebook, and Instagram login walls and age gates")
        .build();

    let cookie_list = gtk::StringList::new(&[]);
    for src in BrowserCookieSource::all() {
        cookie_list.append(&src.short_toolbar_label());
    }

    let initial_cookie_idx = {
        let cur = settings_rc.borrow().cookie_source;
        if cur.is_available_on_system() {
            cur.to_index()
        } else {
            BrowserCookieSource::None.to_index()
        }
    };

    let cookie_combo = adw::ComboRow::builder()
        .title("Browser Cookies")
        .subtitle("Uninstalled browsers (N/A) are disabled")
        .use_markup(false)
        .model(&cookie_list)
        .list_factory(&build_cookie_list_factory(false))
        .selected(initial_cookie_idx)
        .build();

    let last_valid_idx = Rc::new(Cell::new(initial_cookie_idx));
    let settings_for_cookie = settings_rc.clone();
    let on_changed_cookie = on_changed_rc.clone();
    let last_valid_for_cb = last_valid_idx.clone();

    cookie_combo.connect_selected_notify(move |combo| {
        let idx = combo.selected();
        let selected_src = BrowserCookieSource::from_index(idx);
        if !selected_src.is_available_on_system() {
            let valid = last_valid_for_cb.get();
            if combo.selected() != valid {
                combo.set_selected(valid);
            }
            return;
        }
        last_valid_for_cb.set(idx);
        {
            let mut cfg = settings_for_cookie.borrow_mut();
            cfg.cookie_source = selected_src;
            cfg.save();
        }
        on_changed_cookie();
    });
    bypass_group.add(&cookie_combo);

    // Custom cookies.txt file row
    let current_cookie_file = settings_rc
        .borrow()
        .custom_cookies_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "No file selected".to_string());

    let cookie_file_row = adw::ActionRow::builder()
        .title("Custom cookies.txt")
        .subtitle(&current_cookie_file)
        .use_markup(false)
        .title_lines(1)
        .subtitle_lines(2)
        .build();

    let browse_cookie_btn = gtk::Button::builder()
        .icon_name("document-open-symbolic")
        .tooltip_text("Select Netscape cookies.txt file")
        .valign(gtk::Align::Center)
        .build();

    let parent_weak_cookie = parent.downgrade();
    let settings_for_cfile = settings_rc.clone();
    let cookie_file_row_clone = cookie_file_row.clone();
    let cookie_combo_clone = cookie_combo.clone();
    let on_changed_cfile = on_changed_rc.clone();

    browse_cookie_btn.connect_clicked(move |_| {
        let Some(win) = parent_weak_cookie.upgrade() else {
            return;
        };
        let file_dialog = gtk::FileDialog::builder()
            .title("Select Netscape cookies.txt File")
            .modal(true)
            .build();

        let settings_inner = settings_for_cfile.clone();
        let row_inner = cookie_file_row_clone.clone();
        let combo_inner = cookie_combo_clone.clone();
        let cb_inner = on_changed_cfile.clone();

        file_dialog.open(Some(&win), gio::Cancellable::NONE, move |res| {
            if let Ok(file) = res {
                if let Some(path) = file.path() {
                    let norm = metadata::normalize_path(&path);
                    row_inner.set_subtitle(&norm.to_string_lossy());
                    {
                        let mut cfg = settings_inner.borrow_mut();
                        cfg.custom_cookies_path = Some(norm);
                        cfg.cookie_source = BrowserCookieSource::CustomFile;
                        cfg.save();
                    }
                    combo_inner.set_selected(BrowserCookieSource::CustomFile.to_index());
                    cb_inner();
                }
            }
        });
    });

    cookie_file_row.add_suffix(&browse_cookie_btn);
    bypass_group.add(&cookie_file_row);

    // Multi-stage automatic bypass switch
    let auto_bypass_row = adw::SwitchRow::builder()
        .title("Auto Anti-Block Fallbacks")
        .subtitle("Retry blocked downloads with Web/TV/Mobile clients and JS solver")
        .use_markup(false)
        .active(settings_rc.borrow().auto_bypass_fallbacks)
        .build();

    let settings_for_bypass = settings_rc.clone();
    auto_bypass_row.connect_active_notify(move |row| {
        let mut cfg = settings_for_bypass.borrow_mut();
        cfg.auto_bypass_fallbacks = row.is_active();
        cfg.save();
    });
    bypass_group.add(&auto_bypass_row);

    // Proxy EntryRow
    let proxy_row = adw::EntryRow::builder()
        .title("Proxy URL (Optional)")
        .use_markup(false)
        .text(&settings_rc.borrow().proxy_url)
        .build();

    let settings_for_proxy = settings_rc.clone();
    proxy_row.connect_changed(move |entry| {
        let mut cfg = settings_for_proxy.borrow_mut();
        cfg.proxy_url = entry.text().to_string();
        cfg.save();
    });
    bypass_group.add(&proxy_row);
    page.add(&bypass_group);

    // 3. Media Processing & Metadata Group
    let media_group = adw::PreferencesGroup::builder()
        .title("Audio and Video Post-Processing")
        .build();

    let thumb_switch = adw::SwitchRow::builder()
        .title("Embed Cover Art in MP3")
        .subtitle("Embed video thumbnail as album cover art")
        .use_markup(false)
        .active(settings_rc.borrow().embed_thumbnail)
        .build();
    let settings_for_thumb = settings_rc.clone();
    thumb_switch.connect_active_notify(move |row| {
        let mut cfg = settings_for_thumb.borrow_mut();
        cfg.embed_thumbnail = row.is_active();
        cfg.save();
    });
    media_group.add(&thumb_switch);

    let meta_switch = adw::SwitchRow::builder()
        .title("Embed Media Metadata")
        .subtitle("Write title, uploader, and chapters into output files")
        .use_markup(false)
        .active(settings_rc.borrow().embed_metadata)
        .build();
    let settings_for_meta = settings_rc.clone();
    meta_switch.connect_active_notify(move |row| {
        let mut cfg = settings_for_meta.borrow_mut();
        cfg.embed_metadata = row.is_active();
        cfg.save();
    });
    media_group.add(&meta_switch);

    let sb_switch = adw::SwitchRow::builder()
        .title("SponsorBlock Removal")
        .subtitle("Strip sponsor and self-promo segments on YouTube")
        .use_markup(false)
        .active(settings_rc.borrow().sponsorblock_remove)
        .build();
    let settings_for_sb = settings_rc.clone();
    sb_switch.connect_active_notify(move |row| {
        let mut cfg = settings_for_sb.borrow_mut();
        cfg.sponsorblock_remove = row.is_active();
        cfg.save();
    });
    media_group.add(&sb_switch);
    page.add(&media_group);

    // 4. Managed Tools (yt-dlp, ffmpeg, and deno in managed_bin_dir)
    // Split into clean rows so narrow screens (< 398px) never overflow AdwBreakpointBin!
    let versions = InstalledToolVersions::load();
    let bin_dir_str = metadata::managed_bin_dir().to_string_lossy().to_string();
    let bin_display = metadata::managed_bin_dir_display();

    let tools_group = adw::PreferencesGroup::builder()
        .title("Managed Binaries")
        .description(&bin_dir_str)
        .build();

    let deno_status_str = if metadata::managed_deno_path().exists() {
        if versions.deno_version.is_empty() {
            "Installed"
        } else {
            &versions.deno_version
        }
    } else if settings_rc.borrow().use_managed_deno {
        "Pending download"
    } else {
        "System PATH"
    };

    let status_subtitle = format!(
        "yt-dlp: {} • ffmpeg: {} • deno: {}",
        if versions.ytdlp_version.is_empty() {
            "Not installed"
        } else {
            &versions.ytdlp_version
        },
        if versions.ffmpeg_version.is_empty() {
            "Checking"
        } else {
            &versions.ffmpeg_version
        },
        deno_status_str
    );

    let tools_row = adw::ActionRow::builder()
        .title("Installed Versions")
        .subtitle(&status_subtitle)
        .use_markup(false)
        .title_lines(1)
        .subtitle_lines(2)
        .build();

    let open_bin_btn = gtk::Button::builder()
        .icon_name("folder-open-symbolic")
        .tooltip_text(&format!("Open {}", bin_display))
        .valign(gtk::Align::Center)
        .build();
    open_bin_btn.connect_clicked(|_| {
        let _ = open::that(metadata::managed_bin_dir());
    });
    tools_row.add_suffix(&open_bin_btn);
    tools_group.add(&tools_row);

    // Managed Deno switch row (enabled by default; if disabled, system binaries are prioritized;
    // if enabled later, deno is immediately downloaded into managed_bin_dir if missing)
    let managed_deno_switch = adw::SwitchRow::builder()
        .title(&format!("Use Managed Deno in {}", bin_display))
        .subtitle("Enabled by default; disable to prioritize system JS runtimes")
        .use_markup(false)
        .active(settings_rc.borrow().use_managed_deno)
        .build();

    let settings_for_deno = settings_rc.clone();
    let updater_for_deno = on_trigger_updater_rc.clone();
    managed_deno_switch.connect_active_notify(move |row| {
        let enabled = row.is_active();
        {
            let mut cfg = settings_for_deno.borrow_mut();
            cfg.use_managed_deno = enabled;
            cfg.save();
        }
        if enabled && !metadata::managed_deno_path().exists() {
            updater_for_deno(false);
        }
    });
    tools_group.add(&managed_deno_switch);

    let update_action_row = adw::ActionRow::builder()
        .title("Check for Tool Updates")
        .subtitle("Update yt-dlp, ffmpeg, and deno if newer versions exist (or download if missing)")
        .use_markup(false)
        .title_lines(1)
        .subtitle_lines(2)
        .build();

    let force_update_btn = gtk::Button::builder()
        .icon_name("software-update-available-symbolic")
        .label("Update")
        .valign(gtk::Align::Center)
        .build();
    force_update_btn.add_css_class("suggested-action");

    let dialog_for_update = dialog.clone();
    let updater_for_force = on_trigger_updater_rc.clone();
    force_update_btn.connect_clicked(move |_| {
        updater_for_force(false);
        dialog_for_update.close();
    });

    update_action_row.add_suffix(&force_update_btn);
    tools_group.add(&update_action_row);
    page.add(&tools_group);

    dialog.add(&page);
    dialog.present(Some(parent));
}
