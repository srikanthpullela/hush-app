mod dnd;

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconEvent},
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};

static IS_HUSHED: AtomicBool = AtomicBool::new(false);
static PLAY_SOUND: AtomicBool = AtomicBool::new(true);

fn set_tray_icon(app: &AppHandle, icon_file: &str, tooltip: &str) {
    if let Some(tray) = app.tray_by_id("hush-tray") {
        if let Ok(img) = Image::from_path(
            app.path()
                .resolve(icon_file, tauri::path::BaseDirectory::Resource)
                .unwrap_or_default(),
        ) {
            let _ = tray.set_icon(Some(img));
        }
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

fn update_tray_icon(app: &AppHandle, hushed: bool) {
    if hushed {
        set_tray_icon(app, "icons/tray-hushed.png", "Hush — Notifications Off");
    } else {
        set_tray_icon(app, "icons/tray-normal.png", "Hush — Notifications On");
    }
}

fn show_setup_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("setup") {
        let _ = win.show();
        let _ = win.set_focus();
    } else {
        // Window not found in config — create dynamically
        match WebviewWindowBuilder::new(
            app,
            "setup",
            WebviewUrl::App("index.html".into()),
        )
        .title("Hush — Setup")
        .inner_size(520.0, 700.0)
        .resizable(false)
        .center()
        .visible(true)
        .build()
        {
            Ok(win) => {
                let _ = win.show();
                let _ = win.set_focus();
            }
            Err(e) => {
                eprintln!("Failed to create setup window: {e}");
            }
        }
    }
}

fn toggle_hush(app: &AppHandle, force_state: Option<bool>) {
    // Check if shortcuts exist before attempting toggle
    if needs_setup() {
        show_setup_window(app);
        return;
    }

    let current = IS_HUSHED.load(Ordering::Relaxed);
    let new_state = force_state.unwrap_or(!current);
    if new_state == current {
        return;
    }

    // Show loading spinner on tray while shortcut runs
    IS_HUSHED.store(new_state, Ordering::Relaxed);
    set_tray_icon(app, "icons/tray-loading.png", "Hush — Switching…");

    let app_handle = app.clone();
    std::thread::spawn(move || {
        let success = dnd::set_dnd(new_state);
        if success {
            update_tray_icon(&app_handle, new_state);
            if PLAY_SOUND.load(Ordering::Relaxed) {
                play_sound(new_state);
            }
        } else {
            // Revert on failure
            IS_HUSHED.store(!new_state, Ordering::Relaxed);
            update_tray_icon(&app_handle, !new_state);
        }
    });
}

fn play_sound(hushed: bool) {
    #[cfg(target_os = "macos")]
    {
        let sound = if hushed {
            "/System/Library/Sounds/Purr.aiff"
        } else {
            "/System/Library/Sounds/Blow.aiff"
        };
        let _ = std::process::Command::new("/usr/bin/afplay")
            .arg(sound)
            .spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = hushed; // Windows system sounds handled differently
        let _ = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "[System.Media.SystemSounds]::Beep.Play()",
            ])
            .spawn();
    }
}

// MARK: - Tauri Commands for Setup UI

#[derive(serde::Serialize)]
struct ShortcutStatus {
    has_on: bool,
    has_off: bool,
}

#[tauri::command]
fn check_shortcuts() -> ShortcutStatus {
    let (has_on, has_off) = dnd::check_shortcuts();
    ShortcutStatus { has_on, has_off }
}

#[tauri::command]
fn try_auto_setup() -> bool {
    dnd::try_auto_create_shortcuts()
}

#[tauri::command]
fn open_shortcuts_app() {
    dnd::open_shortcuts_app();
}

#[tauri::command]
fn setup_complete(app: AppHandle) {
    // Hide setup window — shortcuts are now configured
    if let Some(win) = app.get_webview_window("setup") {
        let _ = win.hide();
    }
    eprintln!("[Hush] Setup complete — ready for manual toggle");
}

fn needs_setup() -> bool {
    let (has_on, has_off) = dnd::check_shortcuts();
    eprintln!("[Hush] check_shortcuts: has_on={has_on}, has_off={has_off}");
    !(has_on && has_off)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            check_shortcuts,
            try_auto_setup,
            open_shortcuts_app,
            setup_complete,
        ])
        .setup(|app| {
            // Set up tray icon click handler
            if let Some(tray) = app.tray_by_id("hush-tray") {
                tray.on_tray_icon_event(move |tray, event| {
                    match event {
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } => {
                            toggle_hush(tray.app_handle(), None);
                        }
                        TrayIconEvent::Click {
                            button: MouseButton::Right,
                            button_state: MouseButtonState::Up,
                            ..
                        } => {
                            show_menu(tray.app_handle());
                        }
                        _ => {}
                    }
                });
            }

            eprintln!("[Hush] App setup starting...");
            if needs_setup() {
                eprintln!("[Hush] Shortcuts missing — showing setup window");
                // Window is auto-created from config but hidden; show it
                show_setup_window(app.handle());
            } else {
                eprintln!("[Hush] Shortcuts found — ready for manual toggle");
                // Hide setup window since shortcuts exist
                if let Some(win) = app.get_webview_window("setup") {
                    let _ = win.hide();
                }
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, _event| {});
}

fn show_menu(app: &AppHandle) {
    let hushed = IS_HUSHED.load(Ordering::Relaxed);

    let status_text = if hushed {
        "🔕 Notifications Off"
    } else {
        "🔔 Notifications On"
    };
    let toggle_text = if hushed {
        "Turn Notifications On"
    } else {
        "Hush Notifications"
    };

    let status = MenuItem::with_id(app, "status", status_text, false, None::<&str>).unwrap();
    let toggle = MenuItem::with_id(app, "toggle", toggle_text, true, None::<&str>).unwrap();
    let sep1 = PredefinedMenuItem::separator(app).unwrap();
    let sep2 = PredefinedMenuItem::separator(app).unwrap();

    let sound = CheckMenuItem::with_id(
        app,
        "play_sound",
        "Play Sound on Toggle",
        true,
        PLAY_SOUND.load(Ordering::Relaxed),
        None::<&str>,
    )
    .unwrap();

    let quit = MenuItem::with_id(app, "quit", "Quit Hush", true, Some("CmdOrCtrl+Q")).unwrap();

    let menu = Menu::with_items(
        app,
        &[&status, &sep1, &toggle, &sep2, &sound, &sep2, &quit],
    )
    .unwrap();

    if let Some(tray) = app.tray_by_id("hush-tray") {
        let _ = tray.set_menu(Some(menu));
    }

    app.on_menu_event(move |app_h, event: MenuEvent| match event.id().as_ref() {
        "toggle" => toggle_hush(app_h, None),
        "play_sound" => {
            let current = PLAY_SOUND.load(Ordering::Relaxed);
            PLAY_SOUND.store(!current, Ordering::Relaxed);
        }
        "quit" => {
            if IS_HUSHED.load(Ordering::Relaxed) {
                let _ = dnd::set_dnd(false);
            }
            app_h.exit(0);
        }
        _ => {}
    });
}

