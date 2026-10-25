#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod downloader;
pub mod metadata;
pub mod models;
pub mod ui;
pub mod updater;

use adw::prelude::*;
use gtk::{gdk, glib};

fn main() -> glib::ExitCode {
    // Windows DirectWrite/Pango/Vulkan crash mitigation and GIO TLS / Pixbuf module environment setup
    #[cfg(windows)]
    {
        if std::env::var("GSK_RENDERER").is_err() {
            std::env::set_var("GSK_RENDERER", "gl");
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let gio_mods = parent.join("lib").join("gio").join("modules");
                if gio_mods.exists() && std::env::var_os("GIO_MODULE_DIR").is_none() {
                    std::env::set_var("GIO_MODULE_DIR", &gio_mods);
                }
                let ca_bundle = parent.join("ssl").join("certs").join("ca-bundle.crt");
                if ca_bundle.exists() {
                    if std::env::var_os("GTLS_SYSTEM_CA_FILE").is_none() {
                        std::env::set_var("GTLS_SYSTEM_CA_FILE", &ca_bundle);
                    }
                    if std::env::var_os("SSL_CERT_FILE").is_none() {
                        std::env::set_var("SSL_CERT_FILE", &ca_bundle);
                    }
                }
                let pixbuf_cache = parent
                    .join("lib")
                    .join("gdk-pixbuf-2.0")
                    .join("2.10.0")
                    .join("loaders.cache");
                if pixbuf_cache.exists() && std::env::var_os("GDK_PIXBUF_MODULE_FILE").is_none() {
                    std::env::set_var("GDK_PIXBUF_MODULE_FILE", &pixbuf_cache);
                }
            }
        }
    }

    let app = adw::Application::builder()
        .application_id(metadata::APP_ID)
        .build();

    app.connect_startup(|_| {
        let settings = models::AppSettings::load();
        ui::window::apply_theme(settings.dark_theme);
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
