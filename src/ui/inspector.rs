use crate::downloader;
use crate::metadata;
use crate::models::{AppSettings, QualityPreset, VideoMetadata};
use crate::updater;
use adw::prelude::*;
use gtk::{gdk, gio, glib};
use std::cell::RefCell;
use std::rc::Rc;

pub fn show_url_inspector_dialog<F>(
    parent: &impl IsA<gtk::Widget>,
    url: String,
    settings: AppSettings,
    on_enqueue: F,
) where
    F: Fn(String, String, String, QualityPreset, Option<String>, Option<u32>) + 'static,
{
    let dialog = adw::Dialog::builder()
        .title("Inspect Media and Formats")
        .content_width(640)
        .content_height(600)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header);

    let root_stack = gtk::Stack::new();
    root_stack.set_transition_type(gtk::StackTransitionType::Crossfade);

    // 1. Loading State
    let loading_scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let loading_box = gtk::Box::new(gtk::Orientation::Vertical, 16);
    loading_box.set_valign(gtk::Align::Center);
    loading_box.set_halign(gtk::Align::Center);
    loading_box.set_margin_top(36);
    loading_box.set_margin_bottom(36);
    loading_box.set_margin_start(16);
    loading_box.set_margin_end(16);

    let spinner = gtk::Spinner::new();
    spinner.set_size_request(42, 42);
    spinner.start();

    let loading_title = gtk::Label::new(Some("Analyzing URL and bypassing site restrictions..."));
    loading_title.add_css_class("title-3");
    loading_title.set_wrap(true);
    loading_title.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    loading_title.set_width_chars(1);
    loading_title.set_justify(gtk::Justification::Center);

    let loading_sub = gtk::Label::new(Some(
        "Fetching available video resolutions, audio streams, and playlist items via yt-dlp.",
    ));
    loading_sub.add_css_class("dim-label");
    loading_sub.set_wrap(true);
    loading_sub.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    loading_sub.set_width_chars(1);
    loading_sub.set_justify(gtk::Justification::Center);

    loading_box.append(&spinner);
    loading_box.append(&loading_title);
    loading_box.append(&loading_sub);
    loading_scrolled.set_child(Some(&loading_box));
    root_stack.add_named(&loading_scrolled, Some("loading"));

    // 2. Content State wrapped in adw::Clamp for responsive sizing
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let clamp = adw::Clamp::builder()
        .maximum_size(600)
        .tightening_threshold(420)
        .build();

    let content_box = gtk::Box::new(gtk::Orientation::Vertical, 14);
    content_box.set_margin_top(12);
    content_box.set_margin_bottom(16);
    content_box.set_margin_start(12);
    content_box.set_margin_end(12);
    clamp.set_child(Some(&content_box));
    scrolled.set_child(Some(&clamp));
    root_stack.add_named(&scrolled, Some("content"));

    // 3. Error State
    let error_scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    let error_status = adw::StatusPage::builder()
        .icon_name("dialog-error-symbolic")
        .title("Could Not Inspect URL")
        .description("Try selecting your browser cookies if this video requires login or age verification.")
        .build();
    error_scrolled.set_child(Some(&error_status));
    root_stack.add_named(&error_scrolled, Some("error"));

    root_stack.set_visible_child_name("loading");
    toolbar_view.set_content(Some(&root_stack));
    dialog.set_child(Some(&toolbar_view));
    dialog.present(Some(parent));

    let url_for_fetch = url.clone();
    let settings_for_fetch = settings.clone();
    let dialog_weak = dialog.downgrade();
    let on_enqueue_rc = Rc::new(on_enqueue);

    glib::MainContext::default().spawn_local(async move {
        let res = gio::spawn_blocking(move || {
            downloader::inspect_url_blocking(&url_for_fetch, &settings_for_fetch)
        })
        .await;

        let Some(dialog) = dialog_weak.upgrade() else {
            return;
        };

        match res {
            Ok(Ok(meta)) => {
                populate_inspector_content(
                    &dialog,
                    &content_box,
                    meta,
                    on_enqueue_rc.clone(),
                );
                root_stack.set_visible_child_name("content");
            }
            Ok(Err(err_msg)) => {
                error_status.set_description(Some(&err_msg));
                root_stack.set_visible_child_name("error");
            }
            Err(_) => {
                error_status.set_description(Some("Background inspection task failed unexpectedly."));
                root_stack.set_visible_child_name("error");
            }
        }
    });
}

fn populate_inspector_content<F>(
    dialog: &adw::Dialog,
    content_box: &gtk::Box,
    meta: VideoMetadata,
    on_enqueue: Rc<F>,
) where
    F: Fn(String, String, String, QualityPreset, Option<String>, Option<u32>) + 'static,
{
    let is_playlist = meta.is_playlist || !meta.playlist_entries.is_empty();
    let total_playlist_videos = meta.playlist_entries.len();

    // Top Media Preview Card (adapts Horizontal <-> Vertical on narrow dialogs)
    let preview_card = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    preview_card.add_css_class("card");
    preview_card.set_margin_bottom(4);

    let thumb_frame = gtk::Frame::new(None);
    thumb_frame.set_size_request(160, 92);
    thumb_frame.set_halign(gtk::Align::Center);
    thumb_frame.set_margin_top(10);
    thumb_frame.set_margin_bottom(10);
    thumb_frame.set_margin_start(10);
    thumb_frame.set_margin_end(10);

    let picture = gtk::Picture::new();
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_size_request(160, 92);
    thumb_frame.set_child(Some(&picture));
    preview_card.append(&thumb_frame);

    // Load thumbnail asynchronously using libsoup3
    if let Some(ref thumb_url) = meta.thumbnail_url {
        let thumb_url_clone = thumb_url.clone();
        let pic_weak = picture.downgrade();
        glib::MainContext::default().spawn_local(async move {
            if let Ok(bytes) = updater::fetch_bytes_with_soup(&thumb_url_clone).await {
                if let Some(pic) = pic_weak.upgrade() {
                    if let Ok(texture) = gdk::Texture::from_bytes(&bytes) {
                        pic.set_paintable(Some(&texture));
                    }
                }
            }
        });
    }

    let info_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    info_box.set_hexpand(true);
    info_box.set_valign(gtk::Align::Center);
    info_box.set_margin_top(10);
    info_box.set_margin_bottom(10);
    info_box.set_margin_start(10);
    info_box.set_margin_end(12);

    let badges_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let platform_badge = gtk::Label::new(Some(&meta.platform_badge()));
    platform_badge.add_css_class("pill-badge");
    platform_badge.set_ellipsize(gtk::pango::EllipsizeMode::End);
    platform_badge.set_width_chars(1);
    badges_row.append(&platform_badge);

    if is_playlist {
        let count_badge = gtk::Label::new(Some(&format!(
            "{} Videos in Playlist",
            total_playlist_videos.max(1)
        )));
        count_badge.add_css_class("pill-badge-accent");
        count_badge.set_ellipsize(gtk::pango::EllipsizeMode::End);
        count_badge.set_width_chars(1);
        badges_row.append(&count_badge);
    } else {
        let duration_badge = gtk::Label::new(Some(&meta.formatted_duration()));
        duration_badge.add_css_class("dim-label");
        duration_badge.set_ellipsize(gtk::pango::EllipsizeMode::End);
        badges_row.append(&duration_badge);
    }

    let title_label = gtk::Label::new(Some(&meta.title));
    title_label.add_css_class("title-4");
    title_label.set_halign(gtk::Align::Start);
    title_label.set_xalign(0.0);
    title_label.set_wrap(true);
    title_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    title_label.set_lines(3);
    title_label.set_width_chars(1);
    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let uploader_label = gtk::Label::new(Some(&format!("Creator: {}", meta.uploader)));
    uploader_label.add_css_class("dim-label");
    uploader_label.set_halign(gtk::Align::Start);
    uploader_label.set_xalign(0.0);
    uploader_label.set_wrap(true);
    uploader_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    uploader_label.set_width_chars(1);
    uploader_label.set_ellipsize(gtk::pango::EllipsizeMode::End);

    info_box.append(&badges_row);
    info_box.append(&title_label);
    info_box.append(&uploader_label);
    preview_card.append(&info_box);

    content_box.append(&preview_card);

    // Format / Quality Selection Group
    let format_group = adw::PreferencesGroup::builder()
        .title("Select Format and Resolution")
        .description("Choose a recommended preset or a specific stream extracted from the site")
        .build();

    let mut choices: Vec<QualityPreset> = Vec::new();
    for preset in QualityPreset::all_standard() {
        choices.push(preset.clone());
    }

    for fmt in meta.formats.iter().take(15) {
        let label = fmt.display_label();
        choices.push(QualityPreset::CustomFormat {
            format_id: fmt.format_id.clone(),
            label,
            is_audio_only: fmt.is_audio_only,
        });
    }

    let string_list = gtk::StringList::new(&[]);
    for c in &choices {
        string_list.append(&c.display_name());
    }

    let format_row = adw::ComboRow::builder()
        .title("Quality / Stream")
        .subtitle("Merged automatically via ffmpeg")
        .use_markup(false)
        .model(&string_list)
        .selected(0)
        .build();
    format_group.add(&format_row);
    content_box.append(&format_group);

    let initial_btn_label = if is_playlist {
        format!("Download All {} Videos", total_playlist_videos.max(1))
    } else {
        "Add to Queue".to_string()
    };
    let download_btn = gtk::Button::with_label(&initial_btn_label);
    download_btn.set_hexpand(true);
    download_btn.add_css_class("suggested-action");
    download_btn.add_css_class("pill");

    // Interactive Playlist Video Selector & Summary if URL is a playlist
    let playlist_entry_ref: Rc<RefCell<Option<gtk::Entry>>> = Rc::new(RefCell::new(None));
    let playlist_checks_ref: Rc<RefCell<Vec<(usize, gtk::CheckButton)>>> =
        Rc::new(RefCell::new(Vec::new()));

    if is_playlist {
        let pl_section = gtk::Box::new(gtk::Orientation::Vertical, 8);

        let pl_header_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let pl_title_vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
        pl_title_vbox.set_hexpand(true);

        let pl_header_title = gtk::Label::new(Some(&format!(
            "Playlist Videos ({} Total)",
            total_playlist_videos
        )));
        pl_header_title.add_css_class("heading");
        pl_header_title.set_halign(gtk::Align::Start);
        pl_header_title.set_xalign(0.0);
        pl_header_title.set_wrap(true);
        pl_header_title.set_width_chars(1);

        let safe_folder_preview = metadata::sanitize_fs_name(&meta.title, 48, 144);
        let pl_selected_sub = gtk::Label::new(Some(&format!(
            "Selected: {} of {} videos • Saved in folder: {}/",
            total_playlist_videos, total_playlist_videos, safe_folder_preview
        )));
        pl_selected_sub.add_css_class("dim-label");
        pl_selected_sub.set_halign(gtk::Align::Start);
        pl_selected_sub.set_xalign(0.0);
        pl_selected_sub.set_wrap(true);
        pl_selected_sub.set_width_chars(1);

        pl_title_vbox.append(&pl_header_title);
        pl_title_vbox.append(&pl_selected_sub);

        let sel_btns_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        sel_btns_box.set_valign(gtk::Align::Center);

        let select_all_btn = gtk::Button::with_label("Select All");
        let deselect_all_btn = gtk::Button::with_label("None");
        sel_btns_box.append(&select_all_btn);
        sel_btns_box.append(&deselect_all_btn);

        pl_header_row.append(&pl_title_vbox);
        pl_header_row.append(&sel_btns_box);
        pl_section.append(&pl_header_row);

        let pl_entry = gtk::Entry::builder()
            .text(&format!("1-{}", total_playlist_videos.max(1)))
            .valign(gtk::Align::Center)
            .width_chars(10)
            .build();

        if !meta.playlist_entries.is_empty() {
            let list_frame = gtk::Frame::new(None);
            list_frame.add_css_class("card");

            let list_scroll = gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .min_content_height(175)
                .max_content_height(230)
                .propagate_natural_height(true)
                .build();

            let items_vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);

            let update_selection_ui: Rc<dyn Fn()> = {
                let checks_rc = playlist_checks_ref.clone();
                let sub_lbl = pl_selected_sub.clone();
                let dl_btn = download_btn.clone();
                let range_entry = pl_entry.clone();
                let total = total_playlist_videos;
                let folder_name = safe_folder_preview.clone();

                Rc::new(move || {
                    let borrowed = checks_rc.borrow();
                    let selected_indices: Vec<usize> = borrowed
                        .iter()
                        .filter(|(_, cb)| cb.is_active())
                        .map(|(idx, _)| *idx)
                        .collect();
                    let count = selected_indices.len();
                    sub_lbl.set_text(&format!(
                        "Selected: {} of {} videos • Saved in folder: {}/",
                        count, total, folder_name
                    ));
                    if count == 0 {
                        dl_btn.set_label("Select at Least 1 Video");
                        dl_btn.set_sensitive(false);
                        range_entry.set_text("");
                    } else if count == total {
                        dl_btn.set_label(&format!("Download All {} Videos", total));
                        dl_btn.set_sensitive(true);
                        range_entry.set_text(&format!("1-{}", total));
                    } else {
                        dl_btn.set_label(&format!(
                            "Download {} of {} Videos",
                            count, total
                        ));
                        dl_btn.set_sensitive(true);
                        let csv = selected_indices
                            .iter()
                            .map(|i| i.to_string())
                            .collect::<Vec<_>>()
                            .join(",");
                        range_entry.set_text(&csv);
                    }
                })
            };

            for (pos, entry) in meta.playlist_entries.iter().enumerate() {
                if pos > 0 {
                    items_vbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
                }

                let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                row_box.set_margin_top(7);
                row_box.set_margin_bottom(7);
                row_box.set_margin_start(10);
                row_box.set_margin_end(10);

                let check = gtk::CheckButton::new();
                check.set_active(true);
                check.set_valign(gtk::Align::Center);

                let idx_badge = gtk::Label::new(Some(&format!("#{}", entry.index)));
                idx_badge.add_css_class("pill-badge");
                idx_badge.set_valign(gtk::Align::Center);

                let item_title = gtk::Label::new(Some(&entry.title));
                item_title.set_halign(gtk::Align::Start);
                item_title.set_xalign(0.0);
                item_title.set_hexpand(true);
                item_title.set_wrap(true);
                item_title.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                item_title.set_lines(2);
                item_title.set_width_chars(1);
                item_title.set_ellipsize(gtk::pango::EllipsizeMode::End);

                let dur_label = gtk::Label::new(Some(&entry.duration_str));
                dur_label.add_css_class("dim-label");
                dur_label.set_valign(gtk::Align::Center);

                row_box.append(&check);
                row_box.append(&idx_badge);
                row_box.append(&item_title);
                row_box.append(&dur_label);

                let upd = update_selection_ui.clone();
                check.connect_toggled(move |_| {
                    upd();
                });

                playlist_checks_ref
                    .borrow_mut()
                    .push((entry.index, check));
                items_vbox.append(&row_box);
            }

            {
                let checks_rc = playlist_checks_ref.clone();
                select_all_btn.connect_clicked(move |_| {
                    for (_, cb) in checks_rc.borrow().iter() {
                        cb.set_active(true);
                    }
                });
            }

            {
                let checks_rc = playlist_checks_ref.clone();
                deselect_all_btn.connect_clicked(move |_| {
                    for (_, cb) in checks_rc.borrow().iter() {
                        cb.set_active(false);
                    }
                });
            }

            list_scroll.set_child(Some(&items_vbox));
            list_frame.set_child(Some(&list_scroll));
            pl_section.append(&list_frame);
        }

        let pl_range_group = adw::PreferencesGroup::new();
        let pl_row = adw::ActionRow::builder()
            .title("Custom Item Range")
            .subtitle("Auto-filled by checkboxes above, or type a range like 1-5 or 1,3,5")
            .use_markup(false)
            .build();
        pl_row.add_suffix(&pl_entry);
        pl_range_group.add(&pl_row);
        pl_section.append(&pl_range_group);

        content_box.append(&pl_section);
        *playlist_entry_ref.borrow_mut() = Some(pl_entry);
    }

    // Action Buttons
    let actions_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    actions_box.set_halign(gtk::Align::Fill);
    actions_box.set_margin_top(6);

    let cancel_btn = gtk::Button::with_label("Cancel");
    cancel_btn.set_hexpand(true);
    let dialog_for_cancel = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dialog_for_cancel.close();
    });

    // Single unified breakpoint for narrow inspector dialogs
    if let Ok(cond) = adw::BreakpointCondition::parse("max-width: 520sp") {
        let bp = adw::Breakpoint::new(cond);
        let card_apply = preview_card.clone();
        let act_apply = actions_box.clone();
        bp.connect_apply(move |_| {
            card_apply.set_orientation(gtk::Orientation::Vertical);
            act_apply.set_orientation(gtk::Orientation::Vertical);
        });

        let card_unapply = preview_card.clone();
        let act_unapply = actions_box.clone();
        bp.connect_unapply(move |_| {
            card_unapply.set_orientation(gtk::Orientation::Horizontal);
            act_unapply.set_orientation(gtk::Orientation::Horizontal);
        });
        dialog.add_breakpoint(bp);
    }

    let dialog_for_dl = dialog.clone();
    let url_val = meta.url.clone();
    let title_val = meta.title.clone();
    let platform_val = meta.platform_badge();

    download_btn.connect_clicked(move |_| {
        let idx = format_row.selected() as usize;
        let selected_preset = choices
            .get(idx)
            .cloned()
            .unwrap_or(QualityPreset::BestVideo);

        let pl_items = playlist_entry_ref
            .borrow()
            .as_ref()
            .map(|e| e.text().trim().to_string())
            .filter(|s| !s.is_empty());

        let expected_count = if is_playlist {
            let checked_count = playlist_checks_ref
                .borrow()
                .iter()
                .filter(|(_, cb)| cb.is_active())
                .count();
            if checked_count > 0 {
                Some(checked_count as u32)
            } else {
                pl_items.as_ref().map(|spec| estimate_playlist_spec_count(spec))
            }
        } else {
            None
        };

        on_enqueue(
            url_val.clone(),
            title_val.clone(),
            platform_val.clone(),
            selected_preset,
            pl_items,
            expected_count,
        );
        dialog_for_dl.close();
    });

    actions_box.append(&cancel_btn);
    actions_box.append(&download_btn);
    content_box.append(&actions_box);
}

fn estimate_playlist_spec_count(spec: &str) -> u32 {
    let mut total = 0_u32;
    for part in spec.split(',') {
        let part = part.trim();
        if let Some((start_s, end_s)) = part.split_once('-') {
            if let (Ok(s), Ok(e)) = (start_s.trim().parse::<u32>(), end_s.trim().parse::<u32>()) {
                if e >= s {
                    total += e - s + 1;
                    continue;
                }
            }
        }
        if part.parse::<u32>().is_ok() {
            total += 1;
        }
    }
    total.max(1)
}
