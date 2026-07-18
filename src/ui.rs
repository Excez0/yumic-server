use adw::prelude::*;
use std::sync::mpsc::{Sender, Receiver};
use std::time::Duration;

use crate::backend::{BackendCommand, BackendEvent};
use crate::config::{Config, Theme};
use crate::meter::AudioMeter;
use crate::tray::YuMicTray;

const PIPE_PATH: &str = "/tmp/yumic_audio_pipe";

pub fn build_ui(app: &adw::Application) {
    let config = Config::load();
    apply_theme(&config.theme);

    let (event_tx, event_rx): (Sender<BackendEvent>, Receiver<BackendEvent>) = std::sync::mpsc::channel();
    let (cmd_tx, cmd_rx): (Sender<BackendCommand>, Receiver<BackendCommand>) = std::sync::mpsc::channel();
    let (ui_tx, ui_rx) = std::sync::mpsc::channel::<crate::tray::UiCommand>();

    crate::backend::spawn_backend(event_tx, cmd_rx, PIPE_PATH, &config.source_name);

    // Create system tray
    let tray = YuMicTray::new(cmd_tx.clone(), ui_tx);
    let tray_handle = tray.start();

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("YuMic")
        .default_width(420)
        .default_height(560)
        .build();

    // Minimize to tray (hide instead of close)
    window.connect_close_request(move |win| {
        win.set_visible(false);
        glib::Propagation::Stop
    });

    // Poll for UI commands (e.g. from tray menu)
    let window_weak = window.downgrade();
    let app_clone = app.clone();
    let cmd_tx_ref = cmd_tx.clone();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        while let Ok(cmd) = ui_rx.try_recv() {
            match cmd {
                crate::tray::UiCommand::ToggleVisibility => {
                    if let Some(w) = window_weak.upgrade() {
                        if w.is_visible() {
                            w.set_visible(false);
                        } else {
                            w.set_visible(true);
                            w.present();
                        }
                    }
                }
                crate::tray::UiCommand::Quit => {
                    // Send disconnect to backend to clean up PulseAudio modules cleanly!
                    let _ = cmd_tx_ref.send(BackendCommand::Disconnect);
                    // Give backend a brief moment to run PulseAudio cleanup, then quit
                    let app_to_quit = app_clone.clone();
                    glib::timeout_add_local_once(Duration::from_millis(300), move || {
                        app_to_quit.quit();
                    });
                }
            }
        }
        glib::ControlFlow::Continue
    });

    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let header = adw::HeaderBar::new();
    let title_widget = adw::WindowTitle::new("YuMic", "Linux Microphone Server");
    header.set_title_widget(Some(&title_widget));
    main_box.append(&header);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_hscrollbar_policy(gtk::PolicyType::Never);
    scrolled.set_vscrollbar_policy(gtk::PolicyType::Automatic);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(16);
    content.set_margin_end(16);

    // ─── Connection ───
    let conn_group = adw::PreferencesGroup::builder().title("Connection").build();

    let ip_row = adw::EntryRow::builder()
        .title("Phone IP Address")
        .text(&config.phone_ip)
        .build();

    let connect_button = gtk::Button::builder()
        .label("Connect")
        .css_classes(["suggested-action", "pill"])
        .width_request(120)
        .build();

    let status_label = gtk::Label::builder()
        .label("● Disconnected")
        .css_classes(["dim-label", "caption"])
        .halign(gtk::Align::Start)
        .build();

    let conn_inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let ip_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    ip_container.append(&ip_row);
    let btn_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    btn_container.set_valign(gtk::Align::Center);
    btn_container.append(&connect_button);
    conn_inner.append(&ip_container);
    conn_inner.append(&btn_container);

    conn_group.add(&conn_inner);
    conn_group.add(&status_label);
    content.append(&conn_group);

    // Connect button handler
    {
        let cmd_tx_connect = cmd_tx.clone();
        let ip_row_ref = ip_row.clone();
        let status_label_ref = status_label.clone();
        let connect_button_ref = connect_button.clone();

        connect_button.connect_clicked(move |btn| {
            let is_disconnect = btn.label().as_deref() == Some("Disconnect");

            if is_disconnect {
                let _ = cmd_tx_connect.send(BackendCommand::Disconnect);
                btn.set_label("Connect");
                btn.remove_css_class("destructive-action");
                btn.add_css_class("suggested-action");
                status_label_ref.set_text("● Disconnected");
                status_label_ref.remove_css_class("success");
                status_label_ref.add_css_class("dim-label");
            } else {
                let ip = ip_row_ref.text().to_string();
                btn.set_label("Disconnect");
                btn.remove_css_class("suggested-action");
                btn.add_css_class("destructive-action");
                status_label_ref.set_text("● Connecting...");
                status_label_ref.remove_css_class("dim-label");
                status_label_ref.add_css_class("accent");
                btn.set_sensitive(false);

                let status_label2 = status_label_ref.clone();
                let connect_button2 = connect_button_ref.clone();
                let cmd_tx2 = cmd_tx_connect.clone();

                glib::timeout_add_local_once(Duration::from_secs(3), move || {
                    connect_button2.set_sensitive(true);
                    if status_label2.text().as_str() == "● Connecting..." {
                        status_label2.set_text("● Connection failed");
                        status_label2.remove_css_class("accent");
                        status_label2.add_css_class("error");
                        connect_button2.set_label("Connect");
                        connect_button2.remove_css_class("destructive-action");
                        connect_button2.add_css_class("suggested-action");
                    }
                });

                let _ = cmd_tx2.send(BackendCommand::Connect {
                    ip,
                    control_port: 8125,
                    media_port: 49152,
                });
            }
        });
    }

    // ─── Audio ───
    let audio_group = adw::PreferencesGroup::builder().title("Audio").build();

    let level_bar = AudioMeter::new();

    let level_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    level_box.append(&level_bar.widget);
    audio_group.add(&level_box);

    let stats_label = gtk::Label::builder()
        .label("Packets: 0 | Errors: 0 | Rate: 0 KB/s")
        .css_classes(["dim-label", "caption"])
        .halign(gtk::Align::Start)
        .build();
    audio_group.add(&stats_label);
    content.append(&audio_group);

    // ─── Settings ───
    let settings_group = adw::PreferencesGroup::builder().title("Settings").build();

    let _control_port_row = adw::SpinRow::builder()
        .title("Control Port (TCP)")
        .subtitle("Default: 8125")
        .adjustment(&gtk::Adjustment::new(config.control_port as f64, 1024.0, 65535.0, 1.0, 10.0, 0.0))
        .build();

    let _media_port_row = adw::SpinRow::builder()
        .title("Media Port (UDP)")
        .subtitle("Default: 49152")
        .adjustment(&gtk::Adjustment::new(config.media_port as f64, 1024.0, 65535.0, 1.0, 10.0, 0.0))
        .build();

    let _source_row = adw::EntryRow::builder()
        .title("PulseAudio Source Name")
        .text(&config.source_name)
        .build();

    let _auto_row = adw::SwitchRow::builder()
        .title("Auto-connect on startup")
        .active(config.auto_connect)
        .build();

    settings_group.add(&_control_port_row);
    settings_group.add(&_media_port_row);
    settings_group.add(&_source_row);
    settings_group.add(&_auto_row);
    content.append(&settings_group);

    // ─── Theme ───
    let theme_group = adw::PreferencesGroup::builder().title("Appearance").build();

    let theme_row = adw::ComboRow::builder()
        .title("Theme")
        .subtitle("Choose between system, light, or dark theme")
        .model(&gtk::StringList::new(&["System", "Light", "Dark"]))
        .selected(match config.theme {
            Theme::System => 0,
            Theme::Light => 1,
            Theme::Dark => 2,
        })
        .build();

    theme_row.connect_selected_notify(move |row| {
        let theme = match row.selected() {
            0 => Theme::System,
            1 => Theme::Light,
            2 => Theme::Dark,
            _ => Theme::System,
        };
        apply_theme(&theme);
        let mut cfg = Config::load();
        cfg.theme = theme;
        let _ = cfg.save();
    });

    theme_group.add(&theme_row);
    content.append(&theme_group);

    scrolled.set_child(Some(&content));
    main_box.append(&scrolled);

    let footer = gtk::Label::builder()
        .label("YuMic v0.2.0")
        .css_classes(["dim-label", "caption"])
        .margin_top(8)
        .margin_bottom(8)
        .build();
    main_box.append(&footer);

    window.set_content(Some(&main_box));

    // Auto-connect
    if config.auto_connect {
        let ip = config.phone_ip.clone();
        let control_port = config.control_port;
        let media_port = config.media_port;
        glib::timeout_add_local_once(Duration::from_millis(500), move || {
            let _ = cmd_tx.send(BackendCommand::Connect { ip, control_port, media_port });
        });
    }

    // Poll for backend events
    let status_label = status_label.clone();
    let connect_button = connect_button.clone();
    let level_bar = level_bar.clone();
    let stats_label = stats_label.clone();
    let footer = footer.clone();
    let tray_clone = tray_handle.clone();

    glib::timeout_add_local(Duration::from_millis(50), move || {
        while let Ok(event) = event_rx.try_recv() {
            match event {
                BackendEvent::Connected => {
                    status_label.set_text("● Connected");
                    status_label.remove_css_class("dim-label");
                    status_label.remove_css_class("accent");
                    status_label.remove_css_class("error");
                    status_label.add_css_class("success");
                    connect_button.set_label("Disconnect");
                    connect_button.remove_css_class("suggested-action");
                    connect_button.add_css_class("destructive-action");
                    connect_button.set_sensitive(true);
                    footer.set_text("YuMic v0.2.0 — Connected");
                    tray_clone.set_connected();
                }
                BackendEvent::Disconnected => {
                    status_label.set_text("● Disconnected");
                    status_label.remove_css_class("success");
                    status_label.remove_css_class("accent");
                    status_label.remove_css_class("error");
                    status_label.add_css_class("dim-label");
                    connect_button.set_label("Connect");
                    connect_button.remove_css_class("destructive-action");
                    connect_button.add_css_class("suggested-action");
                    connect_button.set_sensitive(true);
                    level_bar.set_level(0.0);
                    level_bar.set_peak(0.0);
                    footer.set_text("YuMic v0.2.0");
                    tray_clone.set_disconnected();
                }
                BackendEvent::Error(msg) => {
                    // Bağlantı hatası = Bağlantı kesildi gibi işle (UI tamamen disconnected olur)
                    status_label.set_text("● Disconnected");
                    status_label.remove_css_class("success");
                    status_label.remove_css_class("accent");
                    status_label.remove_css_class("error");
                    status_label.add_css_class("dim-label");
                    connect_button.set_label("Connect");
                    connect_button.remove_css_class("destructive-action");
                    connect_button.add_css_class("suggested-action");
                    connect_button.set_sensitive(true);
                    level_bar.set_level(0.0);
                    level_bar.set_peak(0.0);
                    footer.set_text(&format!("Error: {}", msg));
                    tray_clone.set_disconnected(); // Tray ikonunu disconnected yap
                }
                BackendEvent::AudioLevel(level) => {
                    level_bar.set_level(level);
                }
                BackendEvent::Stats { packets, errors, bytes } => {
                    let rate = bytes as f64 / 1024.0 / 3.0;
                    stats_label.set_text(&format!(
                        "Packets: {} | Errors: {} | Rate: {:.1} KB/s",
                        packets, errors, rate
                    ));
                }
            }
        }
        glib::ControlFlow::Continue
    });

    window.present();
}

fn apply_theme(theme: &Theme) {
    let style_manager = adw::StyleManager::default();
    match theme {
        Theme::System => style_manager.set_color_scheme(adw::ColorScheme::Default),
        Theme::Light => style_manager.set_color_scheme(adw::ColorScheme::ForceLight),
        Theme::Dark => style_manager.set_color_scheme(adw::ColorScheme::ForceDark),
    }
}
