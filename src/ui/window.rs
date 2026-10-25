use crate::downloader;
use crate::metadata;
use crate::models::{
    detect_platform_from_url, is_likely_playlist_url, AppSettings, BrowserCookieSource,
    CompletedItem, DownloadProgressEvent, DownloadTask, QualityPreset, ToolUpdateEvent,
};
use crate::ui::about::show_about_dialog;
use crate::ui::inspector::show_url_inspector_dialog;
use crate::ui::preferences::{build_cookie_list_factory, show_preferences_dialog};
use crate::updater;
use adw::prelude::*;
use gtk::{gdk, gio, glib};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

struct QueueRowWidgets {
    card: gtk::Box,
    playlist_badge: gtk::Label,
    title_label: gtk::Label,
    current_video_label: gtk::Label,
    playlist_progress_bar: gtk::ProgressBar,
    status_label: gtk::Label,
    speed_eta_label: gtk::Label,
    progress_bar: gtk::ProgressBar,
    cancel_btn: gtk::Button,
    retry_btn: gtk::Button,
    remove_btn: gtk::Button,
    task: DownloadTask,
    playlist_current_idx: u32,
    playlist_total: u32,
    playlist_completed_count: u32,
}

pub fn apply_theme(dark_theme: bool) {
    let style_manager = adw::StyleManager::default();
    if dark_theme {
        style_manager.set_color_scheme(adw::ColorScheme::ForceDark);
    } else {
        style_manager.set_color_scheme(adw::ColorScheme::ForceLight);
    }
}

pub fn build_main_window(app: &adw::Application) -> adw::ApplicationWindow {
    let settings_rc = Rc::new(RefCell::new(AppSettings::load()));
    apply_theme(settings_rc.borrow().dark_theme);

    // Ensure saved cookie_source is valid/available on this system
    {
        let mut cfg = settings_rc.borrow_mut();
        if !cfg.cookie_source.is_available_on_system() {
            cfg.cookie_source = BrowserCookieSource::None;
            cfg.save();
        }
    }

    let history_rc = Rc::new(RefCell::new(CompletedItem::load_history()));
    let next_task_id = Rc::new(Cell::new(1_u64));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(metadata::APP_NAME)
        .default_width(960)
        .default_height(720)
        .width_request(320)
        .height_request(420)
        .build();

    let toast_overlay = adw::ToastOverlay::new();
    let toolbar_view = adw::ToolbarView::new();

    // HeaderBar with ViewSwitcher, Update Tools, Downloads Folder, Settings, and About
    let header_bar = adw::HeaderBar::new();
    header_bar.set_show_end_title_buttons(true);
    header_bar.set_show_start_title_buttons(true);

    let view_stack = adw::ViewStack::new();
    let view_switcher = adw::ViewSwitcher::builder()
        .stack(&view_stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();

    let compact_title_label = gtk::Label::new(Some(metadata::APP_NAME));
    compact_title_label.add_css_class("title");
    compact_title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    compact_title_label.set_width_chars(1);

    header_bar.set_title_widget(Some(&view_switcher));

    // Bottom ViewSwitcherBar for narrow/responsive mobile-like widths
    let bottom_switcher_bar = adw::ViewSwitcherBar::builder()
        .stack(&view_stack)
        .reveal(false)
        .build();

    // Left HeaderBar: Open Downloads Folder & Update Tools
    let open_dl_folder_btn = gtk::Button::builder()
        .icon_name("folder-download-symbolic")
        .tooltip_text("Open Downloads Folder")
        .valign(gtk::Align::Center)
        .build();

    let update_tools_btn = gtk::Button::builder()
        .icon_name("software-update-available-symbolic")
        .tooltip_text(&format!(
            "Check and update yt-dlp, ffmpeg, and deno in {}",
            metadata::managed_bin_dir_display()
        ))
        .valign(gtk::Align::Center)
        .build();

    header_bar.pack_start(&open_dl_folder_btn);
    header_bar.pack_start(&update_tools_btn);

    // Right HeaderBar: Settings & About
    let about_btn = gtk::Button::builder()
        .icon_name("help-about-symbolic")
        .tooltip_text("About arvdownloader")
        .valign(gtk::Align::Center)
        .build();

    let settings_btn = gtk::Button::builder()
        .icon_name("emblem-system-symbolic")
        .tooltip_text("Preferences and Anti-Block Settings")
        .valign(gtk::Align::Center)
        .build();

    header_bar.pack_end(&about_btn);
    header_bar.pack_end(&settings_btn);

    toolbar_view.add_top_bar(&header_bar);
    toolbar_view.add_bottom_bar(&bottom_switcher_bar);

    // Main Vertical Layout wrapped in an adw::Clamp for responsive centering on all screen sizes
    let outer_vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer_vbox.set_vexpand(true);
    outer_vbox.set_hexpand(true);

    let top_clamp = adw::Clamp::builder()
        .maximum_size(940)
        .tightening_threshold(600)
        .build();

    let top_vbox = gtk::Box::new(gtk::Orientation::Vertical, 10);
    top_vbox.set_margin_top(10);
    top_vbox.set_margin_bottom(6);
    top_vbox.set_margin_start(12);
    top_vbox.set_margin_end(12);

    // 1. Startup / Tool Updater Progress Card (yt-dlp, ffmpeg & deno via libsoup3)
    let updater_card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    updater_card.add_css_class("card");
    updater_card.add_css_class("updater-banner");

    let updater_top_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    updater_top_row.set_margin_top(10);
    updater_top_row.set_margin_start(12);
    updater_top_row.set_margin_end(12);

    let updater_status_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    updater_status_box.set_hexpand(true);

    let updater_spinner = gtk::Spinner::new();
    updater_spinner.set_valign(gtk::Align::Center);
    updater_spinner.start();

    let updater_status_label = gtk::Label::new(Some(&format!(
        "Checking {} for yt-dlp, ffmpeg, and deno updates...",
        metadata::managed_bin_dir_display()
    )));
    updater_status_label.set_halign(gtk::Align::Start);
    updater_status_label.set_xalign(0.0);
    updater_status_label.set_hexpand(true);
    updater_status_label.set_wrap(true);
    updater_status_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    updater_status_label.set_width_chars(1);
    updater_status_label.add_css_class("heading");

    updater_status_box.append(&updater_spinner);
    updater_status_box.append(&updater_status_label);

    let init_versions = crate::models::InstalledToolVersions::load();
    let init_yt = if init_versions.ytdlp_version.is_empty() {
        "..."
    } else {
        &init_versions.ytdlp_version
    };
    let init_ff = if init_versions.ffmpeg_version.is_empty() {
        "..."
    } else {
        &init_versions.ffmpeg_version
    };
    let init_dn = if init_versions.deno_version.is_empty() {
        "..."
    } else {
        &init_versions.deno_version
    };
    let updater_versions_badge = gtk::Label::new(Some(&format!(
        "yt-dlp: {} • ffmpeg: {} • deno: {}",
        init_yt, init_ff, init_dn
    )));
    updater_versions_badge.add_css_class("pill-badge");
    updater_versions_badge.set_halign(gtk::Align::Start);
    updater_versions_badge.set_valign(gtk::Align::Center);
    updater_versions_badge.set_wrap(true);
    updater_versions_badge.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    updater_versions_badge.set_width_chars(1);

    updater_top_row.append(&updater_status_box);
    updater_top_row.append(&updater_versions_badge);

    let updater_progress_bar = gtk::ProgressBar::new();
    updater_progress_bar.set_show_text(true);
    updater_progress_bar.set_ellipsize(gtk::pango::EllipsizeMode::End);
    updater_progress_bar.set_margin_start(12);
    updater_progress_bar.set_margin_end(12);
    updater_progress_bar.set_margin_bottom(10);
    updater_progress_bar.set_fraction(0.05);
    updater_progress_bar.set_text(Some("Initializing updater..."));

    updater_card.append(&updater_top_row);
    updater_card.append(&updater_progress_bar);
    top_vbox.append(&updater_card);

    // 2. URL Input & Action Card
    let input_card = gtk::Box::new(gtk::Orientation::Vertical, 10);
    input_card.add_css_class("card");

    let url_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    url_row.set_margin_top(12);
    url_row.set_margin_start(12);
    url_row.set_margin_end(12);

    let url_entry = gtk::Entry::builder()
        .placeholder_text(
            "Paste YouTube, Facebook, Instagram, Reels, Shorts, TikTok, or X link...",
        )
        .hexpand(true)
        .width_chars(1)
        .secondary_icon_name("edit-clear-symbolic")
        .secondary_icon_tooltip_text("Clear URL")
        .build();

    url_entry.connect_icon_press(|entry, pos| {
        if pos == gtk::EntryIconPosition::Secondary {
            entry.set_text("");
        }
    });

    let paste_btn = gtk::Button::builder()
        .icon_name("edit-paste-symbolic")
        .label("Paste")
        .tooltip_text("Paste link from clipboard")
        .build();

    let url_entry_for_paste = url_entry.clone();
    paste_btn.connect_clicked(move |_| {
        if let Some(display) = gdk::Display::default() {
            let clipboard = display.clipboard();
            let entry_clone = url_entry_for_paste.clone();
            clipboard.read_text_async(gio::Cancellable::NONE, move |res| {
                if let Ok(Some(text)) = res {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        entry_clone.set_text(trimmed);
                    }
                }
            });
        }
    });

    url_row.append(&url_entry);
    url_row.append(&paste_btn);
    input_card.append(&url_row);

    // Controls Row: [Selectors Box (Quality + Cookies)] + [Action Buttons Box (Inspect + Quick Download)]
    let controls_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    controls_row.set_margin_bottom(12);
    controls_row.set_margin_start(12);
    controls_row.set_margin_end(12);

    let selectors_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    selectors_box.set_hexpand(true);

    let preset_strings = gtk::StringList::new(&[]);
    for p in QualityPreset::all_standard() {
        preset_strings.append(&p.display_name());
    }
    let preset_dropdown = gtk::DropDown::new(Some(preset_strings), gtk::Expression::NONE);
    preset_dropdown.set_selected(settings_rc.borrow().default_preset.to_index());
    preset_dropdown.set_hexpand(true);
    preset_dropdown.set_tooltip_text(Some("Select output video resolution or MP3/M4A audio format"));

    // Quick Browser Cookie Selector (shows full text in popup & button, disables (N/A) items)
    let cookie_strings = gtk::StringList::new(&[]);
    for src in BrowserCookieSource::all() {
        cookie_strings.append(&src.short_toolbar_label());
    }
    let initial_cookie_idx = settings_rc.borrow().cookie_source.to_index();
    let quick_cookie_dropdown = gtk::DropDown::new(Some(cookie_strings), gtk::Expression::NONE);
    quick_cookie_dropdown.set_list_factory(Some(&build_cookie_list_factory(false)));
    quick_cookie_dropdown.set_selected(initial_cookie_idx);
    quick_cookie_dropdown.set_hexpand(true);
    quick_cookie_dropdown.set_tooltip_text(Some(
        "Browser Cookies Bypass (Browsers marked (N/A) are not installed and are disabled)",
    ));

    selectors_box.append(&preset_dropdown);
    selectors_box.append(&quick_cookie_dropdown);

    let action_btns_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);

    let inspect_btn = gtk::Button::builder()
        .icon_name("system-search-symbolic")
        .label("Inspect Formats")
        .hexpand(true)
        .tooltip_text("Fetch thumbnail, resolutions (4K/1080p/720p/MP3), and playlist items before downloading")
        .build();

    let quick_dl_btn = gtk::Button::builder()
        .icon_name("folder-download-symbolic")
        .label("Quick Download")
        .hexpand(true)
        .tooltip_text("Immediately add to download queue with selected quality preset")
        .build();
    quick_dl_btn.add_css_class("suggested-action");

    action_btns_box.append(&inspect_btn);
    action_btns_box.append(&quick_dl_btn);

    controls_row.append(&selectors_box);
    controls_row.append(&action_btns_box);
    input_card.append(&controls_row);

    top_vbox.append(&input_card);
    top_clamp.set_child(Some(&top_vbox));
    outer_vbox.append(&top_clamp);

    // 3. ViewStack: Active Queue & Completed History (each inside a ScrolledWindow with Never horizontal scrollbar)
    // 3a. Active Queue Page
    let queue_page_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    queue_page_box.set_vexpand(true);

    let queue_scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let queue_clamp = adw::Clamp::builder()
        .maximum_size(940)
        .tightening_threshold(600)
        .build();

    let queue_list_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
    queue_list_box.set_margin_top(4);
    queue_list_box.set_margin_bottom(14);
    queue_list_box.set_margin_start(12);
    queue_list_box.set_margin_end(12);
    queue_clamp.set_child(Some(&queue_list_box));
    queue_scrolled.set_child(Some(&queue_clamp));

    let queue_empty_scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    let queue_empty_status = adw::StatusPage::builder()
        .icon_name("folder-download-symbolic")
        .title("Download Queue is Empty")
        .description("Paste a link from YouTube, Facebook, Instagram (Reels/Shorts/Videos) above and click Quick Download or Inspect Formats.")
        .vexpand(true)
        .build();
    queue_empty_scrolled.set_child(Some(&queue_empty_status));

    let queue_stack = gtk::Stack::new();
    queue_stack.set_vexpand(true);
    queue_stack.add_named(&queue_empty_scrolled, Some("empty"));
    queue_stack.add_named(&queue_scrolled, Some("list"));
    queue_stack.set_visible_child_name("empty");
    queue_page_box.append(&queue_stack);

    let queue_stack_page = view_stack.add_titled_with_icon(
        &queue_page_box,
        Some("queue"),
        "Active Queue",
        "folder-download-symbolic",
    );

    // 3b. Completed History Page
    let history_page_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    history_page_box.set_vexpand(true);

    let history_scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let history_clamp = adw::Clamp::builder()
        .maximum_size(940)
        .tightening_threshold(600)
        .build();

    let history_inner_vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
    history_inner_vbox.set_margin_top(4);
    history_inner_vbox.set_margin_bottom(14);
    history_inner_vbox.set_margin_start(12);
    history_inner_vbox.set_margin_end(12);

    let history_header_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let history_title = gtk::Label::new(Some("Completed Downloads"));
    history_title.add_css_class("heading");
    history_title.set_halign(gtk::Align::Start);
    history_title.set_xalign(0.0);
    history_title.set_wrap(true);
    history_title.set_width_chars(1);
    history_title.set_hexpand(true);

    let clear_history_btn = gtk::Button::builder()
        .label("Clear History")
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    history_header_row.append(&history_title);
    history_header_row.append(&clear_history_btn);

    let history_list_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    history_inner_vbox.append(&history_header_row);
    history_inner_vbox.append(&history_list_box);
    history_clamp.set_child(Some(&history_inner_vbox));
    history_scrolled.set_child(Some(&history_clamp));

    let history_empty_scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    let history_empty_status = adw::StatusPage::builder()
        .icon_name("object-select-symbolic")
        .title("No Completed Downloads Yet")
        .description("Finished video and MP3 downloads will appear here with quick buttons to open the file or folder.")
        .vexpand(true)
        .build();
    history_empty_scrolled.set_child(Some(&history_empty_status));

    let history_stack = gtk::Stack::new();
    history_stack.set_vexpand(true);
    history_stack.add_named(&history_empty_scrolled, Some("empty"));
    history_stack.add_named(&history_scrolled, Some("list"));

    history_page_box.append(&history_stack);

    let history_stack_page = view_stack.add_titled_with_icon(
        &history_page_box,
        Some("history"),
        "Completed",
        "object-select-symbolic",
    );

    outer_vbox.append(&view_stack);
    toolbar_view.set_content(Some(&outer_vbox));
    toast_overlay.set_child(Some(&toolbar_view));
    window.set_content(Some(&toast_overlay));

    // Single unified AdwBreakpoint (<= 740sp):
    // Using ONE breakpoint prevents Libadwaita from unapplying a wider breakpoint when entering a narrower one!
    if let Ok(cond) = adw::BreakpointCondition::parse("max-width: 740sp") {
        let bp = adw::Breakpoint::new(cond);
        let header_apply = header_bar.clone();
        let title_apply = compact_title_label.clone();
        let bottom_apply = bottom_switcher_bar.clone();
        let controls_apply = controls_row.clone();
        let selectors_apply = selectors_box.clone();
        let actions_apply = action_btns_box.clone();
        let updater_row_apply = updater_top_row.clone();
        let paste_apply = paste_btn.clone();
        let top_vbox_apply = top_vbox.clone();
        let q_list_apply = queue_list_box.clone();
        let h_vbox_apply = history_inner_vbox.clone();

        bp.connect_apply(move |_| {
            header_apply.set_title_widget(Some(&title_apply));
            bottom_apply.set_reveal(true);
            controls_apply.set_orientation(gtk::Orientation::Vertical);
            selectors_apply.set_orientation(gtk::Orientation::Vertical);
            actions_apply.set_orientation(gtk::Orientation::Vertical);
            updater_row_apply.set_orientation(gtk::Orientation::Vertical);
            paste_apply.set_label("");
            paste_apply.set_icon_name("edit-paste-symbolic");
            top_vbox_apply.set_margin_start(8);
            top_vbox_apply.set_margin_end(8);
            q_list_apply.set_margin_start(8);
            q_list_apply.set_margin_end(8);
            h_vbox_apply.set_margin_start(8);
            h_vbox_apply.set_margin_end(8);
        });

        let header_unapply = header_bar.clone();
        let switcher_unapply = view_switcher.clone();
        let bottom_unapply = bottom_switcher_bar.clone();
        let controls_unapply = controls_row.clone();
        let selectors_unapply = selectors_box.clone();
        let actions_unapply = action_btns_box.clone();
        let updater_row_unapply = updater_top_row.clone();
        let paste_unapply = paste_btn.clone();
        let top_vbox_unapply = top_vbox.clone();
        let q_list_unapply = queue_list_box.clone();
        let h_vbox_unapply = history_inner_vbox.clone();

        bp.connect_unapply(move |_| {
            header_unapply.set_title_widget(Some(&switcher_unapply));
            bottom_unapply.set_reveal(false);
            controls_unapply.set_orientation(gtk::Orientation::Horizontal);
            selectors_unapply.set_orientation(gtk::Orientation::Horizontal);
            actions_unapply.set_orientation(gtk::Orientation::Horizontal);
            updater_row_unapply.set_orientation(gtk::Orientation::Horizontal);
            paste_unapply.set_label("Paste");
            paste_unapply.set_icon_name("edit-paste-symbolic");
            top_vbox_unapply.set_margin_start(12);
            top_vbox_unapply.set_margin_end(12);
            q_list_unapply.set_margin_start(12);
            q_list_unapply.set_margin_end(12);
            h_vbox_unapply.set_margin_start(12);
            h_vbox_unapply.set_margin_end(12);
        });

        window.add_breakpoint(bp);
    }

    // Wire About Dialog Button
    {
        let window_weak = window.downgrade();
        about_btn.connect_clicked(move |_| {
            if let Some(win) = window_weak.upgrade() {
                show_about_dialog(&win);
            }
        });
    }

    // Populate initial completed history UI (verifying files exist and capping at 10 rows)
    let refresh_history_ui: Rc<dyn Fn()> = {
        let history_rc = history_rc.clone();
        let history_list_box = history_list_box.clone();
        let history_stack = history_stack.clone();
        let history_stack_page = history_stack_page.clone();
        let clear_history_btn = clear_history_btn.clone();

        Rc::new(move || {
            while let Some(child) = history_list_box.first_child() {
                history_list_box.remove(&child);
            }
            {
                let mut items_mut = history_rc.borrow_mut();
                if CompletedItem::prune_missing_and_cap(&mut items_mut) {
                    CompletedItem::save_history(&items_mut);
                }
            }
            let items = history_rc.borrow();
            history_stack_page.set_badge_number(items.len() as u32);
            clear_history_btn.set_sensitive(!items.is_empty());

            if items.is_empty() {
                history_stack.set_visible_child_name("empty");
                return;
            }
            history_stack.set_visible_child_name("list");

            for item in items.iter().rev() {
                let card = build_completed_item_card(item);
                history_list_box.append(&card);
            }
        })
    };
    refresh_history_ui();

    // Clear history button handler with confirmation choice dialog
    {
        let window_weak = window.downgrade();
        let history_rc = history_rc.clone();
        let refresh_history_ui = refresh_history_ui.clone();
        clear_history_btn.connect_clicked(move |_| {
            if history_rc.borrow().is_empty() {
                return;
            }
            let Some(win) = window_weak.upgrade() else {
                return;
            };
            let dialog = adw::AlertDialog::builder()
                .heading("Clear Download History?")
                .body("Are you sure you want to clear all items from the completed history list? Your downloaded files on disk will not be deleted.")
                .default_response("cancel")
                .close_response("cancel")
                .build();
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("clear", "Clear History");
            dialog.set_response_appearance("clear", adw::ResponseAppearance::Destructive);

            let history_rc = history_rc.clone();
            let refresh_history_ui = refresh_history_ui.clone();
            dialog.connect_response(None, move |_, response| {
                if response == "clear" {
                    history_rc.borrow_mut().clear();
                    CompletedItem::save_history(&[]);
                    refresh_history_ui();
                }
            });
            dialog.present(Some(&win));
        });
    }

    // Channel and state for active download queue
    let (dl_progress_tx, dl_progress_rx) = async_channel::unbounded::<DownloadProgressEvent>();
    let (job_queue_tx, job_queue_rx) = async_channel::unbounded::<DownloadTask>();
    let queue_rows_map: Rc<RefCell<HashMap<u64, QueueRowWidgets>>> =
        Rc::new(RefCell::new(HashMap::new()));

    // Spawn 2 concurrent background workers to process queued downloads smoothly
    for _ in 0..2 {
        let rx = job_queue_rx.clone();
        let progress_tx = dl_progress_tx.clone();
        let settings_rc_worker = settings_rc.clone();

        glib::MainContext::default().spawn_local(async move {
            while let Ok(task) = rx.recv().await {
                let current_settings = settings_rc_worker.borrow().clone();
                let tx_clone = progress_tx.clone();
                let _ = gio::spawn_blocking(move || {
                    downloader::execute_download_task_blocking(task, current_settings, tx_clone);
                })
                .await;
            }
        });
    }

    // Helper closure to enqueue a new download task
    let enqueue_download: Rc<
        dyn Fn(String, String, String, QualityPreset, Option<String>, Option<u32>),
    > = {
        let next_task_id = next_task_id.clone();
        let settings_rc = settings_rc.clone();
        let queue_list_box = queue_list_box.clone();
        let queue_stack = queue_stack.clone();
        let queue_stack_page = queue_stack_page.clone();
        let queue_rows_map = queue_rows_map.clone();
        let job_queue_tx = job_queue_tx.clone();
        let view_stack = view_stack.clone();

        Rc::new(
            move |url: String,
                  title: String,
                  platform: String,
                  preset: QualityPreset,
                  playlist_items: Option<String>,
                  expected_playlist_count: Option<u32>| {
                let task_id = next_task_id.get();
                next_task_id.set(task_id + 1);

                let output_dir = settings_rc.borrow().download_dir.clone();
                let task = DownloadTask {
                    id: task_id,
                    url: url.clone(),
                    title: title.clone(),
                    platform: platform.clone(),
                    preset: preset.clone(),
                    output_dir,
                    playlist_items,
                    expected_playlist_count,
                };

                let row_widgets = build_queue_row_widgets(
                    &task,
                    &queue_list_box,
                    &queue_stack,
                    &queue_stack_page,
                    &queue_rows_map,
                    &job_queue_tx,
                );

                queue_list_box.prepend(&row_widgets.card);
                queue_rows_map.borrow_mut().insert(task_id, row_widgets);

                queue_stack.set_visible_child_name("list");
                queue_stack_page.set_badge_number(queue_rows_map.borrow().len() as u32);
                view_stack.set_visible_child_name("queue");

                let _ = job_queue_tx.send_blocking(task);
            },
        )
    };

    // Listen for DownloadProgressEvents on the GTK main loop
    {
        let queue_rows_map = queue_rows_map.clone();
        let queue_list_box = queue_list_box.clone();
        let queue_stack = queue_stack.clone();
        let queue_stack_page = queue_stack_page.clone();
        let history_rc = history_rc.clone();
        let refresh_history_ui = refresh_history_ui.clone();
        let toast_overlay = toast_overlay.clone();

        glib::MainContext::default().spawn_local(async move {
            while let Ok(ev) = dl_progress_rx.recv().await {
                match ev {
                    DownloadProgressEvent::Started {
                        task_id,
                        status_text,
                    } => {
                        if let Some(w) = queue_rows_map.borrow().get(&task_id) {
                            w.status_label.set_text(&status_text);
                            w.progress_bar.set_fraction(0.02);
                            w.progress_bar.set_text(Some("Starting..."));
                            w.cancel_btn.set_visible(true);
                            w.retry_btn.set_visible(false);
                            w.remove_btn.set_visible(false);
                        }
                    }
                    DownloadProgressEvent::TitleUpdated { task_id, title } => {
                        if let Some(w) = queue_rows_map.borrow_mut().get_mut(&task_id) {
                            w.title_label.set_text(&title);
                            w.task.title = title;
                        }
                    }
                    DownloadProgressEvent::PlaylistDetected {
                        task_id,
                        playlist_title,
                        total_items,
                    } => {
                        if let Some(w) = queue_rows_map.borrow_mut().get_mut(&task_id) {
                            let safe_folder = metadata::sanitize_fs_name(&playlist_title, 48, 144);
                            w.title_label.set_text(&format!(
                                "Playlist: {} (Folder: {}/)",
                                playlist_title, safe_folder
                            ));
                            w.task.title = playlist_title;
                            if total_items > 0 {
                                w.playlist_total = total_items;
                            }
                            w.playlist_badge.set_visible(true);
                            w.current_video_label.set_visible(true);
                            w.playlist_progress_bar.set_visible(true);

                            if w.playlist_total > 0 {
                                w.playlist_badge.set_text(&format!(
                                    "0 of {} Videos • 0 Done",
                                    w.playlist_total
                                ));
                                w.playlist_progress_bar.set_text(Some(&format!(
                                    "Overall Playlist: 0 / {} Done ({} Left) — 0%",
                                    w.playlist_total, w.playlist_total
                                )));
                            } else {
                                w.playlist_badge.set_text("Playlist");
                                w.playlist_progress_bar
                                    .set_text(Some("Overall Playlist: Preparing items..."));
                            }
                        }
                    }
                    DownloadProgressEvent::PlaylistItemStarted {
                        task_id,
                        current_index,
                        total_items,
                        video_title,
                        playlist_title,
                    } => {
                        if let Some(w) = queue_rows_map.borrow_mut().get_mut(&task_id) {
                            w.playlist_current_idx = current_index;
                            if total_items > 0 {
                                w.playlist_total = total_items;
                            }
                            if let Some(pl_t) = playlist_title {
                                if !pl_t.is_empty() {
                                    let safe_folder = metadata::sanitize_fs_name(&pl_t, 48, 144);
                                    w.title_label.set_text(&format!(
                                        "Playlist: {} (Folder: {}/)",
                                        pl_t, safe_folder
                                    ));
                                    w.task.title = pl_t;
                                }
                            }

                            let total = w.playlist_total.max(current_index).max(1);
                            let done = w.playlist_completed_count;
                            let remaining = total.saturating_sub(done);

                            w.playlist_badge.set_visible(true);
                            w.playlist_badge.set_text(&format!(
                                "Video {} of {} • {} Done",
                                current_index, total, done
                            ));

                            w.current_video_label.set_visible(true);
                            w.current_video_label.set_text(&format!(
                                "Now Downloading [{} / {}]: {}",
                                current_index, total, video_title
                            ));

                            w.playlist_progress_bar.set_visible(true);
                            let pl_frac = (done as f64 / total as f64).clamp(0.0, 1.0);
                            w.playlist_progress_bar.set_fraction(pl_frac);
                            w.playlist_progress_bar.set_text(Some(&format!(
                                "Overall Playlist: {} / {} Done ({} Left) — {:.0}%",
                                done,
                                total,
                                remaining,
                                pl_frac * 100.0
                            )));

                            w.progress_bar.set_fraction(0.02);
                            w.progress_bar.set_text(Some(&format!(
                                "Current Video ({}/{}): Starting...",
                                current_index, total
                            )));
                        }
                    }
                    DownloadProgressEvent::PlaylistItemCompleted {
                        task_id,
                        current_index,
                        total_items,
                        video_title,
                        file_path,
                    } => {
                        let maybe_completed = {
                            let mut map_borrow = queue_rows_map.borrow_mut();
                            if let Some(w) = map_borrow.get_mut(&task_id) {
                                w.playlist_completed_count =
                                    w.playlist_completed_count.max(current_index);
                                w.playlist_total =
                                    w.playlist_total.max(total_items).max(current_index);

                                let total = w.playlist_total.max(1);
                                let done = w.playlist_completed_count.min(total);
                                let remaining = total.saturating_sub(done);
                                let pl_frac = (done as f64 / total as f64).clamp(0.0, 1.0);

                                w.playlist_badge.set_visible(true);
                                w.playlist_badge.set_text(&format!(
                                    "Video {} of {} • {} Done",
                                    current_index, total, done
                                ));

                                w.playlist_progress_bar.set_visible(true);
                                w.playlist_progress_bar.set_fraction(pl_frac);
                                w.playlist_progress_bar.set_text(Some(&format!(
                                    "Overall Playlist: {} / {} Done ({} Left) — {:.0}%",
                                    done,
                                    total,
                                    remaining,
                                    pl_frac * 100.0
                                )));

                                w.progress_bar.set_fraction(1.0);
                                w.progress_bar.set_text(Some(&format!(
                                    "Video {}/{} Completed (100%)",
                                    current_index, total
                                )));

                                Some(CompletedItem {
                                    id: task_id.wrapping_mul(1000).wrapping_add(current_index as u64),
                                    title: format!(
                                        "[{}/{}] {}",
                                        current_index, total, video_title
                                    ),
                                    url: w.task.url.clone(),
                                    platform: w.task.platform.clone(),
                                    quality_badge: w.task.preset.short_badge(),
                                    file_path,
                                    completed_at: "Completed".to_string(),
                                })
                            } else {
                                None
                            }
                        };

                        if let Some(item) = maybe_completed {
                            {
                                let mut hist = history_rc.borrow_mut();
                                CompletedItem::push_and_save(&mut hist, item);
                            }
                            refresh_history_ui();
                        }
                    }
                    DownloadProgressEvent::Progress {
                        task_id,
                        fraction,
                        speed,
                        eta,
                        status_text,
                    } => {
                        if let Some(w) = queue_rows_map.borrow().get(&task_id) {
                            if w.playlist_total > 1 || w.playlist_current_idx > 0 {
                                let idx = w.playlist_current_idx.max(1);
                                let total = w.playlist_total.max(idx);
                                let done = w.playlist_completed_count;
                                let remaining = total.saturating_sub(done);

                                w.progress_bar.set_fraction(fraction);
                                w.progress_bar.set_text(Some(&format!(
                                    "Current Video ({}/{}): {:.1}%",
                                    idx,
                                    total,
                                    fraction * 100.0
                                )));

                                let base_done = idx.saturating_sub(1).max(done.min(idx.saturating_sub(1))) as f64;
                                let overall_pl_frac =
                                    ((base_done + fraction) / (total as f64)).clamp(0.0, 0.999);
                                w.playlist_progress_bar.set_visible(true);
                                w.playlist_progress_bar.set_fraction(overall_pl_frac);
                                w.playlist_progress_bar.set_text(Some(&format!(
                                    "Overall Playlist: {} / {} Done ({} Left) — {:.0}%",
                                    done,
                                    total,
                                    remaining,
                                    overall_pl_frac * 100.0
                                )));
                            } else {
                                w.progress_bar.set_fraction(fraction);
                                w.progress_bar
                                    .set_text(Some(&format!("{:.1}%", fraction * 100.0)));
                            }
                            w.status_label.set_text(&status_text);
                            w.speed_eta_label
                                .set_text(&format!("{} • ETA {}", speed, eta));
                        }
                    }
                    DownloadProgressEvent::BypassRetry {
                        task_id,
                        attempt,
                        strategy_name,
                    } => {
                        if let Some(w) = queue_rows_map.borrow().get(&task_id) {
                            w.status_label.set_text(&format!(
                                "Anti-Block Attempt #{}: {}...",
                                attempt, strategy_name
                            ));
                            w.progress_bar
                                .set_text(Some("Bypassing site restriction..."));
                        }
                    }
                    DownloadProgressEvent::PostProcessing { task_id, step } => {
                        if let Some(w) = queue_rows_map.borrow().get(&task_id) {
                            w.progress_bar.set_fraction(0.99);
                            if w.playlist_total > 1 || w.playlist_current_idx > 0 {
                                let idx = w.playlist_current_idx.max(1);
                                let total = w.playlist_total.max(idx);
                                w.progress_bar.set_text(Some(&format!(
                                    "Current Video ({}/{}): 99% — Finalizing...",
                                    idx, total
                                )));
                            } else {
                                w.progress_bar.set_text(Some("99% — Finalizing..."));
                            }
                            w.status_label.set_text(&step);
                            w.speed_eta_label.set_text("ffmpeg merge");
                        }
                    }
                    DownloadProgressEvent::Completed {
                        task_id,
                        title,
                        file_path,
                    } => {
                        let removed = queue_rows_map.borrow_mut().remove(&task_id);
                        if let Some(w) = removed {
                            queue_list_box.remove(&w.card);
                            let remaining = queue_rows_map.borrow().len();
                            queue_stack_page.set_badge_number(remaining as u32);
                            if remaining == 0 {
                                queue_stack.set_visible_child_name("empty");
                            }

                            // If playlist items were not already added individually, add the single item
                            if w.playlist_completed_count == 0 {
                                let completed_item = CompletedItem {
                                    id: task_id,
                                    title: title.clone(),
                                    url: w.task.url.clone(),
                                    platform: w.task.platform.clone(),
                                    quality_badge: w.task.preset.short_badge(),
                                    file_path,
                                    completed_at: "Completed".to_string(),
                                };
                                {
                                    let mut hist = history_rc.borrow_mut();
                                    CompletedItem::push_and_save(&mut hist, completed_item);
                                }
                                refresh_history_ui();
                            }

                            let summary_text = if w.playlist_completed_count > 1 {
                                format!(
                                    "Playlist finished ({} videos downloaded): {}",
                                    w.playlist_completed_count, title
                                )
                            } else {
                                format!("Download finished: {}", title)
                            };
                            let clean_msg = metadata::sanitize_toast_text(&summary_text);
                            let toast = adw::Toast::builder()
                                .title(clean_msg)
                                .use_markup(false)
                                .build();
                            toast_overlay.add_toast(toast);
                        }
                    }
                    DownloadProgressEvent::Failed { task_id, error } => {
                        if let Some(w) = queue_rows_map.borrow().get(&task_id) {
                            w.status_label.set_text(&format!("Error: {}", error));
                            w.speed_eta_label.set_text("Failed");
                            w.progress_bar.set_text(Some("Failed"));
                            w.cancel_btn.set_visible(false);
                            w.retry_btn.set_visible(true);
                            w.remove_btn.set_visible(true);
                        }
                    }
                    DownloadProgressEvent::Cancelled { task_id } => {
                        if let Some(w) = queue_rows_map.borrow().get(&task_id) {
                            w.status_label.set_text("Cancelled by user");
                            w.speed_eta_label.set_text("Stopped");
                            w.progress_bar.set_fraction(0.0);
                            w.progress_bar.set_text(Some("Cancelled"));
                            w.cancel_btn.set_visible(false);
                            w.retry_btn.set_visible(true);
                            w.remove_btn.set_visible(true);
                        }
                    }
                }
            }
        });
    }

    // Wire Quick Download Button & URL Entry Activate
    let trigger_quick_download: Rc<dyn Fn()> = {
        let url_entry = url_entry.clone();
        let preset_dropdown = preset_dropdown.clone();
        let enqueue_download = enqueue_download.clone();
        let toast_overlay = toast_overlay.clone();

        Rc::new(move || {
            let url = url_entry.text().trim().to_string();
            if url.is_empty() || (!url.starts_with("http://") && !url.starts_with("https://")) {
                toast_overlay.add_toast(adw::Toast::new(
                    "Please enter a valid http:// or https:// media link first.",
                ));
                return;
            }
            let preset = QualityPreset::from_index(preset_dropdown.selected());
            let platform = detect_platform_from_url(&url);
            let is_pl = is_likely_playlist_url(&url);
            let initial_title = if is_pl {
                format!("Fetching playlist from {}...", platform)
            } else {
                format!("Fetching media from {}...", platform)
            };
            let pl_items = if is_pl {
                Some("all".to_string())
            } else {
                None
            };

            enqueue_download(url, initial_title, platform, preset, pl_items, None);
            url_entry.set_text("");
        })
    };

    {
        let trigger = trigger_quick_download.clone();
        quick_dl_btn.connect_clicked(move |_| {
            trigger();
        });
    }
    {
        let trigger = trigger_quick_download.clone();
        url_entry.connect_activate(move |_| {
            trigger();
        });
    }

    // Wire Inspect Formats Button
    {
        let window_weak = window.downgrade();
        let url_entry = url_entry.clone();
        let settings_rc = settings_rc.clone();
        let enqueue_download = enqueue_download.clone();
        let toast_overlay = toast_overlay.clone();

        inspect_btn.connect_clicked(move |_| {
            let url = url_entry.text().trim().to_string();
            if url.is_empty() || (!url.starts_with("http://") && !url.starts_with("https://")) {
                toast_overlay.add_toast(adw::Toast::new(
                    "Please paste a valid http:// or https:// video link to inspect.",
                ));
                return;
            }
            let Some(win) = window_weak.upgrade() else {
                return;
            };
            let current_settings = settings_rc.borrow().clone();
            let enqueue_cb = enqueue_download.clone();
            let entry_to_clear = url_entry.clone();

            show_url_inspector_dialog(
                &win,
                url,
                current_settings,
                move |u, t, p, preset, pl, expected_count| {
                    enqueue_cb(u, t, p, preset, pl, expected_count);
                    entry_to_clear.set_text("");
                },
            );
        });
    }

    // Wire Quick Cookie Dropdown (guards against selecting disabled N/A items via scroll/keyboard)
    let last_valid_cookie_idx = Rc::new(Cell::new(initial_cookie_idx));
    {
        let settings_rc = settings_rc.clone();
        let last_valid_cookie_idx = last_valid_cookie_idx.clone();
        quick_cookie_dropdown.connect_selected_notify(move |dd| {
            let idx = dd.selected();
            let src = BrowserCookieSource::from_index(idx);
            if !src.is_available_on_system() {
                let valid = last_valid_cookie_idx.get();
                if dd.selected() != valid {
                    dd.set_selected(valid);
                }
                return;
            }
            last_valid_cookie_idx.set(idx);
            let mut cfg = settings_rc.borrow_mut();
            if cfg.cookie_source != src {
                cfg.cookie_source = src;
                cfg.save();
            }
        });
    }

    // Wire Preset Dropdown to persist default preset
    {
        let settings_rc = settings_rc.clone();
        preset_dropdown.connect_selected_notify(move |dd| {
            let preset = QualityPreset::from_index(dd.selected());
            let mut cfg = settings_rc.borrow_mut();
            cfg.default_preset = preset;
            cfg.save();
        });
    }

    // Wire Open Downloads Folder Button
    {
        let settings_rc = settings_rc.clone();
        open_dl_folder_btn.connect_clicked(move |_| {
            let dir = settings_rc.borrow().download_dir.clone();
            let _ = std::fs::create_dir_all(&dir);
            let _ = open::that(dir);
        });
    }

    // Track whether core tools (yt-dlp / ffmpeg / deno) are currently being downloaded/updated
    let tools_updating = Rc::new(Cell::new(false));
    let force_close_confirmed = Rc::new(Cell::new(false));

    // Updater Runner Closure (runs on startup and when "Update Tools" is clicked)
    let trigger_tools_updater: Rc<dyn Fn(bool)> = {
        let updater_card = updater_card.clone();
        let updater_spinner = updater_spinner.clone();
        let updater_status_label = updater_status_label.clone();
        let updater_versions_badge = updater_versions_badge.clone();
        let updater_progress_bar = updater_progress_bar.clone();
        let update_tools_btn = update_tools_btn.clone();
        let toast_overlay = toast_overlay.clone();
        let tools_updating = tools_updating.clone();

        Rc::new(move |force_reinstall: bool| {
            tools_updating.set(true);
            updater_card.set_visible(true);
            updater_progress_bar.set_visible(true);
            updater_spinner.set_visible(true);
            updater_spinner.start();
            update_tools_btn.set_sensitive(false);

            let (tx, rx) = async_channel::unbounded::<ToolUpdateEvent>();

            glib::MainContext::default().spawn_local(async move {
                updater::run_tools_updater(tx, force_reinstall).await;
            });

            let updater_spinner = updater_spinner.clone();
            let updater_status_label = updater_status_label.clone();
            let updater_versions_badge = updater_versions_badge.clone();
            let updater_progress_bar = updater_progress_bar.clone();
            let update_tools_btn = update_tools_btn.clone();
            let toast_overlay = toast_overlay.clone();
            let tools_updating = tools_updating.clone();

            glib::MainContext::default().spawn_local(async move {
                while let Ok(ev) = rx.recv().await {
                    match ev {
                        ToolUpdateEvent::Started { message } => {
                            updater_status_label.set_text(&message);
                            updater_progress_bar.set_fraction(0.05);
                            updater_progress_bar.set_text(Some("Starting check..."));
                        }
                        ToolUpdateEvent::Progress {
                            tool_name,
                            fraction,
                            status_text,
                        } => {
                            updater_status_label.set_text(&status_text);
                            updater_progress_bar.set_fraction(fraction);
                            updater_progress_bar.set_text(Some(&format!(
                                "{}: {:.0}%",
                                tool_name,
                                fraction * 100.0
                            )));
                        }
                        ToolUpdateEvent::Warning { message } => {
                            updater_status_label.set_text(&message);
                        }
                        ToolUpdateEvent::Completed {
                            ytdlp_version,
                            ffmpeg_version,
                            deno_version,
                            updated_any,
                        } => {
                            tools_updating.set(false);
                            updater_spinner.stop();
                            updater_spinner.set_visible(false);
                            updater_progress_bar.set_fraction(1.0);
                            updater_progress_bar.set_text(Some("100% — Ready"));
                            updater_progress_bar.set_visible(false);
                            update_tools_btn.set_sensitive(true);

                            updater_status_label.set_text(&format!(
                                "yt-dlp, ffmpeg, and deno ready in {}",
                                metadata::managed_bin_dir_display()
                            ));
                            updater_versions_badge.set_text(&format!(
                                "yt-dlp: {} • ffmpeg: {} • deno: {}",
                                ytdlp_version, ffmpeg_version, deno_version
                            ));

                            if updated_any {
                                let toast = adw::Toast::builder()
                                    .title("yt-dlp, ffmpeg, and deno have been updated to the latest release.")
                                    .use_markup(false)
                                    .build();
                                toast_overlay.add_toast(toast);
                            }
                        }
                        ToolUpdateEvent::Error { message } => {
                            tools_updating.set(false);
                            updater_spinner.stop();
                            updater_spinner.set_visible(false);
                            updater_progress_bar.set_fraction(0.0);
                            updater_progress_bar.set_text(Some("Update Error"));
                            update_tools_btn.set_sensitive(true);
                            updater_status_label.set_text(&message);
                        }
                    }
                }
                tools_updating.set(false);
            });
        })
    };

    // Wire HeaderBar Update Tools Button
    {
        let trigger_updater = trigger_tools_updater.clone();
        update_tools_btn.connect_clicked(move |_| {
            trigger_updater(false);
        });
    }

    // Wire Settings Button
    {
        let window_weak = window.downgrade();
        let settings_rc = settings_rc.clone();
        let quick_cookie_dropdown = quick_cookie_dropdown.clone();
        let last_valid_cookie_idx = last_valid_cookie_idx.clone();
        let trigger_updater = trigger_tools_updater.clone();

        settings_btn.connect_clicked(move |_| {
            let Some(win) = window_weak.upgrade() else {
                return;
            };
            let settings_for_sync = settings_rc.clone();
            let dd_for_sync = quick_cookie_dropdown.clone();
            let last_valid_sync = last_valid_cookie_idx.clone();
            let updater_cb = trigger_updater.clone();

            show_preferences_dialog(
                &win,
                settings_rc.clone(),
                move || {
                    let idx = settings_for_sync.borrow().cookie_source.to_index();
                    last_valid_sync.set(idx);
                    if dd_for_sync.selected() != idx {
                        dd_for_sync.set_selected(idx);
                    }
                },
                move |force_reinstall: bool| {
                    updater_cb(force_reinstall);
                },
            );
        });
    }

    // Confirm before closing if any video/playlist download or tool download (yt-dlp / ffmpeg / deno) is running
    {
        let queue_rows_map = queue_rows_map.clone();
        let tools_updating = tools_updating.clone();
        let force_close_confirmed = force_close_confirmed.clone();
        let close_dialog_open = Rc::new(Cell::new(false));

        window.connect_close_request(move |win| {
            if force_close_confirmed.get() {
                return glib::Propagation::Proceed;
            }

            let active_video_count = queue_rows_map
                .borrow()
                .values()
                .filter(|w| w.cancel_btn.is_visible())
                .count();
            let has_video_download = active_video_count > 0 || downloader::has_active_downloads();
            let has_tool_download = tools_updating.get();

            if !has_video_download && !has_tool_download {
                return glib::Propagation::Proceed;
            }

            if close_dialog_open.get() {
                return glib::Propagation::Stop;
            }
            close_dialog_open.set(true);

            let (heading, body) = match (has_video_download, has_tool_download) {
                (true, true) => (
                    "Active Downloads in Progress",
                    "A video download and a core tool download (yt-dlp, ffmpeg, or deno) are currently running.\n\n\
                     Closing arvdownloader now will cancel these downloads. Are you sure you want to quit?"
                        .to_string(),
                ),
                (true, false) => (
                    "Video Download in Progress",
                    format!(
                        "There {} currently {} active video/playlist download(s) running.\n\n\
                         Closing arvdownloader now will abort the download. Are you sure you want to quit?",
                        if active_video_count.max(1) == 1 { "is" } else { "are" },
                        active_video_count.max(1)
                    ),
                ),
                (false, true) => (
                    "Tool Download in Progress",
                    format!(
                        "Core media tools (yt-dlp, ffmpeg, or deno) are currently being downloaded into {}.\n\n\
                         Closing arvdownloader now may leave the tool installation incomplete. Are you sure you want to quit?",
                        metadata::managed_bin_dir_display()
                    ),
                ),
                (false, false) => unreachable!(),
            };

            let dialog = adw::AlertDialog::builder()
                .heading(heading)
                .body(&body)
                .default_response("cancel")
                .close_response("cancel")
                .build();
            dialog.add_response("cancel", "Keep Running");
            dialog.add_response("quit", "Quit Anyway");
            dialog.set_response_appearance("quit", adw::ResponseAppearance::Destructive);

            let win_clone = win.clone();
            let force_close_confirmed = force_close_confirmed.clone();
            let close_dialog_open = close_dialog_open.clone();
            dialog.connect_response(None, move |_, response| {
                close_dialog_open.set(false);
                if response == "quit" {
                    downloader::cancel_all_tasks();
                    force_close_confirmed.set(true);
                    win_clone.close();
                }
            });

            dialog.present(Some(win));
            glib::Propagation::Stop
        });
    }

    // Automatically run the libsoup3 tool updater on startup
    trigger_tools_updater(false);

    window
}

fn build_queue_row_widgets(
    task: &DownloadTask,
    queue_list_box: &gtk::Box,
    queue_stack: &gtk::Stack,
    queue_stack_page: &adw::ViewStackPage,
    queue_rows_map: &Rc<RefCell<HashMap<u64, QueueRowWidgets>>>,
    job_queue_tx: &async_channel::Sender<DownloadTask>,
) -> QueueRowWidgets {
    let initial_playlist_total = task.expected_playlist_count.unwrap_or(0);
    let is_playlist_task = initial_playlist_total > 1 || task.playlist_items.is_some();

    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.add_css_class("card");

    // Top row: Badges on the left, Cancel/Retry/Remove action buttons on the right
    let header_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    header_row.set_margin_top(10);
    header_row.set_margin_start(12);
    header_row.set_margin_end(12);

    let badges_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    badges_box.set_hexpand(true);
    badges_box.set_valign(gtk::Align::Center);

    let platform_badge = gtk::Label::new(Some(&task.platform));
    platform_badge.add_css_class("pill-badge");
    platform_badge.set_ellipsize(gtk::pango::EllipsizeMode::End);
    platform_badge.set_width_chars(1);

    let quality_badge = gtk::Label::new(Some(&task.preset.short_badge()));
    quality_badge.add_css_class("pill-badge-accent");
    quality_badge.set_ellipsize(gtk::pango::EllipsizeMode::End);
    quality_badge.set_width_chars(1);

    let initial_pl_badge_text = if initial_playlist_total > 0 {
        format!("0 of {} Videos • 0 Done", initial_playlist_total)
    } else {
        "Playlist".to_string()
    };
    let playlist_badge = gtk::Label::new(Some(&initial_pl_badge_text));
    playlist_badge.add_css_class("pill-badge");
    playlist_badge.set_ellipsize(gtk::pango::EllipsizeMode::End);
    playlist_badge.set_width_chars(1);
    playlist_badge.set_visible(is_playlist_task);

    badges_box.append(&platform_badge);
    badges_box.append(&quality_badge);
    badges_box.append(&playlist_badge);

    let cancel_btn = gtk::Button::builder()
        .icon_name("process-stop-symbolic")
        .tooltip_text("Cancel Download")
        .valign(gtk::Align::Center)
        .build();
    cancel_btn.add_css_class("destructive-action");

    let retry_btn = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Retry Download")
        .visible(false)
        .valign(gtk::Align::Center)
        .build();

    let remove_btn = gtk::Button::builder()
        .icon_name("user-trash-symbolic")
        .tooltip_text("Remove from Queue")
        .visible(false)
        .valign(gtk::Align::Center)
        .build();

    header_row.append(&badges_box);
    header_row.append(&retry_btn);
    header_row.append(&remove_btn);
    header_row.append(&cancel_btn);

    // Title label on its own full-width line with WordChar wrapping so narrow screens never clip it
    let display_title = if is_playlist_task
        && !task.title.starts_with("Playlist:")
        && !task.title.starts_with("Fetching")
    {
        let safe_folder = metadata::sanitize_fs_name(&task.title, 48, 144);
        format!("Playlist: {} (Folder: {}/)", task.title, safe_folder)
    } else {
        task.title.clone()
    };
    let title_label = gtk::Label::new(Some(&display_title));
    title_label.add_css_class("heading");
    title_label.set_halign(gtk::Align::Start);
    title_label.set_xalign(0.0);
    title_label.set_hexpand(true);
    title_label.set_wrap(true);
    title_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    title_label.set_lines(2);
    title_label.set_width_chars(1);
    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title_label.set_margin_start(12);
    title_label.set_margin_end(12);

    // Current video label (shown for playlists so user sees the exact video title and index being downloaded)
    let current_video_label = gtk::Label::new(Some("Preparing playlist items..."));
    current_video_label.set_halign(gtk::Align::Start);
    current_video_label.set_xalign(0.0);
    current_video_label.set_hexpand(true);
    current_video_label.set_wrap(true);
    current_video_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    current_video_label.set_lines(2);
    current_video_label.set_width_chars(1);
    current_video_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    current_video_label.set_margin_start(12);
    current_video_label.set_margin_end(12);
    current_video_label.set_visible(is_playlist_task);

    // Overall Playlist Progress Bar (shown above the current video progress bar for playlists)
    let playlist_progress_bar = gtk::ProgressBar::new();
    playlist_progress_bar.set_show_text(true);
    playlist_progress_bar.set_ellipsize(gtk::pango::EllipsizeMode::End);
    playlist_progress_bar.set_fraction(0.0);
    playlist_progress_bar.set_text(Some(&if initial_playlist_total > 0 {
        format!(
            "Overall Playlist: 0 / {} Done ({} Left) — 0%",
            initial_playlist_total, initial_playlist_total
        )
    } else {
        "Overall Playlist: Queued...".to_string()
    }));
    playlist_progress_bar.set_margin_start(12);
    playlist_progress_bar.set_margin_end(12);
    playlist_progress_bar.set_visible(is_playlist_task);

    let progress_bar = gtk::ProgressBar::new();
    progress_bar.set_show_text(true);
    progress_bar.set_ellipsize(gtk::pango::EllipsizeMode::End);
    progress_bar.set_fraction(0.0);
    progress_bar.set_text(Some("Queued..."));
    progress_bar.set_margin_start(12);
    progress_bar.set_margin_end(12);

    let bottom_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    bottom_box.set_margin_bottom(10);
    bottom_box.set_margin_start(12);
    bottom_box.set_margin_end(12);

    let status_label = gtk::Label::new(Some("Waiting in queue..."));
    status_label.add_css_class("dim-label");
    status_label.set_halign(gtk::Align::Start);
    status_label.set_xalign(0.0);
    status_label.set_hexpand(true);
    status_label.set_wrap(true);
    status_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    status_label.set_width_chars(1);

    let speed_eta_label = gtk::Label::new(Some("-- MB/s • ETA --:--"));
    speed_eta_label.add_css_class("dim-label");
    speed_eta_label.set_halign(gtk::Align::Start);
    speed_eta_label.set_xalign(0.0);
    speed_eta_label.set_wrap(true);
    speed_eta_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    speed_eta_label.set_width_chars(1);

    bottom_box.append(&status_label);
    bottom_box.append(&speed_eta_label);

    card.append(&header_row);
    card.append(&title_label);
    card.append(&current_video_label);
    card.append(&playlist_progress_bar);
    card.append(&progress_bar);
    card.append(&bottom_box);

    let task_id = task.id;
    cancel_btn.connect_clicked(move |_| {
        downloader::cancel_task(task_id);
    });

    {
        let queue_rows_map = queue_rows_map.clone();
        let job_queue_tx = job_queue_tx.clone();
        retry_btn.connect_clicked(move |_| {
            let maybe_task = {
                let mut map_borrow = queue_rows_map.borrow_mut();
                if let Some(w) = map_borrow.get_mut(&task_id) {
                    w.playlist_current_idx = 0;
                    w.playlist_completed_count = 0;
                    w.status_label.set_text("Re-queued...");
                    w.progress_bar.set_fraction(0.0);
                    w.progress_bar.set_text(Some("Queued..."));
                    w.playlist_progress_bar.set_fraction(0.0);
                    w.cancel_btn.set_visible(true);
                    w.retry_btn.set_visible(false);
                    w.remove_btn.set_visible(false);
                    Some(w.task.clone())
                } else {
                    None
                }
            };
            if let Some(t) = maybe_task {
                let _ = job_queue_tx.send_blocking(t);
            }
        });
    }

    {
        let queue_rows_map = queue_rows_map.clone();
        let queue_list_box = queue_list_box.clone();
        let queue_stack = queue_stack.clone();
        let queue_stack_page = queue_stack_page.clone();

        remove_btn.connect_clicked(move |_| {
            let removed = queue_rows_map.borrow_mut().remove(&task_id);
            if let Some(w) = removed {
                queue_list_box.remove(&w.card);
            }
            let remaining = queue_rows_map.borrow().len();
            queue_stack_page.set_badge_number(remaining as u32);
            if remaining == 0 {
                queue_stack.set_visible_child_name("empty");
            }
        });
    }

    QueueRowWidgets {
        card,
        playlist_badge,
        title_label,
        current_video_label,
        playlist_progress_bar,
        status_label,
        speed_eta_label,
        progress_bar,
        cancel_btn,
        retry_btn,
        remove_btn,
        task: task.clone(),
        playlist_current_idx: 0,
        playlist_total: initial_playlist_total,
        playlist_completed_count: 0,
    }
}

fn build_completed_item_card(item: &CompletedItem) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.add_css_class("card");

    let top_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    top_row.set_margin_top(10);
    top_row.set_margin_start(12);
    top_row.set_margin_end(12);

    let badges_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    badges_row.set_hexpand(true);
    badges_row.set_valign(gtk::Align::Center);

    let platform_badge = gtk::Label::new(Some(&item.platform));
    platform_badge.add_css_class("pill-badge");
    platform_badge.set_ellipsize(gtk::pango::EllipsizeMode::End);
    platform_badge.set_width_chars(1);

    let quality_badge = gtk::Label::new(Some(&item.quality_badge));
    quality_badge.add_css_class("pill-badge-accent");
    quality_badge.set_ellipsize(gtk::pango::EllipsizeMode::End);
    quality_badge.set_width_chars(1);

    badges_row.append(&platform_badge);
    badges_row.append(&quality_badge);

    let btns_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    btns_box.set_valign(gtk::Align::Center);

    let open_file_btn = gtk::Button::builder()
        .icon_name("media-playback-start-symbolic")
        .tooltip_text("Play / Open File")
        .build();

    let file_path_for_open: PathBuf = item.file_path.clone();
    open_file_btn.connect_clicked(move |_| {
        let _ = open::that(&file_path_for_open);
    });

    let open_folder_btn = gtk::Button::builder()
        .icon_name("folder-open-symbolic")
        .tooltip_text("Open Containing Folder")
        .build();

    let folder_path: PathBuf = item
        .file_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| item.file_path.clone());
    open_folder_btn.connect_clicked(move |_| {
        let _ = open::that(&folder_path);
    });

    btns_box.append(&open_file_btn);
    btns_box.append(&open_folder_btn);

    top_row.append(&badges_row);
    top_row.append(&btns_box);

    let title_label = gtk::Label::new(Some(&item.title));
    title_label.add_css_class("heading");
    title_label.set_halign(gtk::Align::Start);
    title_label.set_xalign(0.0);
    title_label.set_hexpand(true);
    title_label.set_wrap(true);
    title_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    title_label.set_lines(2);
    title_label.set_width_chars(1);
    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title_label.set_margin_start(12);
    title_label.set_margin_end(12);

    let path_label = gtk::Label::new(Some(&item.file_path.to_string_lossy()));
    path_label.add_css_class("dim-label");
    path_label.set_halign(gtk::Align::Start);
    path_label.set_xalign(0.0);
    path_label.set_hexpand(true);
    path_label.set_wrap(true);
    path_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    path_label.set_lines(2);
    path_label.set_width_chars(1);
    path_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    path_label.set_margin_start(12);
    path_label.set_margin_end(12);
    path_label.set_margin_bottom(10);

    card.append(&top_row);
    card.append(&title_label);
    card.append(&path_label);
    card
}
