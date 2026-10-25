#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod downloader;
pub mod metadata;
pub mod models;
pub mod ui;
pub mod updater;

use adw::prelude::*;
use gtk::{gdk, glib};

fn main() -> glib::ExitCode {
    // Windows DirectWrite/Pango/Vulkan crash mitigation
    #[cfg(windows)]
    {
        if std::env::var("GSK_RENDERER").is_err() {
            std::env::set_var("GSK_RENDERER", "gl");
        }
    }

    let app = adw::Application::builder()
        .application_id(metadata::APP_ID)
        .build();

    app.connect_startup(|_| {
        register_app_icons();
        load_custom_css();
    });

    app.connect_activate(|app| {
        if let Some(win) = app.active_window() {
            win.present();
            return;
        }
        let window = ui::window::build_main_window(app);
        window.present();
    });

    app.run()
}

fn register_app_icons() {
    if let Some(display) = gdk::Display::default() {
        let icon_theme = gtk::IconTheme::for_display(&display);
        let local_icons = std::path::Path::new("data/icons");
        if local_icons.exists() {
            icon_theme.add_search_path(local_icons);
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let bundled_icons = parent.join("data").join("icons");
                if bundled_icons.exists() {
                    icon_theme.add_search_path(&bundled_icons);
                }
                let share_icons = parent.join("share").join("icons");
                if share_icons.exists() {
                    icon_theme.add_search_path(&share_icons);
                }
            }
        }
    }
    gtk::Window::set_default_icon_name(metadata::ICON_NAME);
}

fn load_custom_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        r#"
        .pill-badge {
            background-color: alpha(@accent_bg_color, 0.16);
            color: @accent_color;
            border-radius: 999px;
            padding: 2px 10px;
            font-size: 0.82em;
            font-weight: 700;
        }
        .pill-badge-accent {
            background-color: alpha(@success_bg_color, 0.18);
            color: @success_color;
            border-radius: 999px;
            padding: 2px 10px;
            font-size: 0.82em;
            font-weight: 700;
        }
        .updater-banner {
            border-left: 4px solid @accent_bg_color;
        }
        progressbar text {
            font-weight: 600;
            font-size: 0.85em;
        }
        "#,
    );

    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
