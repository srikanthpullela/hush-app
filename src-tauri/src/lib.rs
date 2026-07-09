mod dnd;
mod meeting;

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconEvent},
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};

static IS_HUSHED: AtomicBool = AtomicBool::new(false);
static PLAY_SOUND: AtomicBool = AtomicBool::new(true);
static AUTO_DND_SCREEN_SHARE: AtomicBool = AtomicBool::new(true);
static AUTO_HUSHED_BY_MEETING: AtomicBool = AtomicBool::new(false);
/// Set when user manually overrides DND during a meeting.
/// Prevents auto re-enabling until screen sharing stops and starts again.
static MANUAL_OVERRIDE: AtomicBool = AtomicBool::new(false);
/// Prevents starting multiple poll loops.
static POLL_STARTED: AtomicBool = AtomicBool::new(false);
/// Prevents overlapping toggle threads (e.g. rapid clicks or poll racing a manual toggle).
static TOGGLE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

fn set_tray_icon(app: &AppHandle, icon_file: &str, tooltip: &str) {
    if let Some(tray) = app.tray_by_id("hush-tray") {
        if let Ok(img) = Image::from_path(
            app.path()
                .resolve(icon_file, tauri::path::BaseDirectory::Resource)
                .unwrap_or_default(),
        ) {
            let _ = tray.set_icon(Some(img));
            // On macOS, mark as template image so the OS automatically renders
            // it white on dark menu bars and black on light menu bars.
            #[cfg(target_os = "macos")]
            let _ = tray.set_icon_as_template(true);
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
    let is_auto = force_state.is_some();

    // Check if shortcuts exist before attempting toggle
    if needs_setup() {
        if is_auto {
            // Auto (poll-driven) toggle: don't steal focus every cycle —
            // just log and keep the auto-hush flag consistent with reality.
            eprintln!("[Hush] Auto-toggle skipped — shortcuts not set up");
            AUTO_HUSHED_BY_MEETING.store(!force_state.unwrap(), Ordering::Relaxed);
        } else {
            show_setup_window(app);
        }
        return;
    }

    // Shortcuts exist — make sure meeting detection is running (covers the
    // case where the user created shortcuts after dismissing the setup window)
    start_meeting_poll(app.clone());

    let current = IS_HUSHED.load(Ordering::Relaxed);
    let new_state = force_state.unwrap_or(!current);
    if new_state == current {
        return;
    }

    // Don't start a second toggle while one is still running
    if TOGGLE_IN_PROGRESS.swap(true, Ordering::Relaxed) {
        eprintln!("[Hush] Toggle already in progress — skipping");
        if is_auto {
            // Keep flag consistent so the poll loop retries next cycle
            AUTO_HUSHED_BY_MEETING.store(!new_state, Ordering::Relaxed);
        }
        return;
    }

    // If this is a manual toggle (force_state == None), set manual override
    // so the poll loop won't re-enable DND during this screen share session
    if force_state.is_none() {
        MANUAL_OVERRIDE.store(true, Ordering::Relaxed);
        AUTO_HUSHED_BY_MEETING.store(false, Ordering::Relaxed);
        eprintln!("[Hush] Manual toggle — auto-hush paused until next meeting");
    }

    // Show loading spinner on tray while shortcut runs
    IS_HUSHED.store(new_state, Ordering::Relaxed);
    set_tray_icon(app, "icons/tray-loading.png", "Hush — Switching…");

    let app_handle = app.clone();
    std::thread::spawn(move || {
        let success = dnd::set_dnd(new_state);
        if success {
            update_tray_icon(&app_handle, new_state);
            build_and_set_menu(&app_handle);
            if PLAY_SOUND.load(Ordering::Relaxed) {
                play_sound(new_state);
            }
        } else {
            // Revert on failure
            IS_HUSHED.store(!new_state, Ordering::Relaxed);
            if is_auto {
                // Restore the auto-hush flag so the poll loop retries:
                // failed auto-ON → not auto-hushed; failed auto-OFF → still auto-hushed
                AUTO_HUSHED_BY_MEETING.store(!new_state, Ordering::Relaxed);
            }
            update_tray_icon(&app_handle, !new_state);
            build_and_set_menu(&app_handle);
        }
        TOGGLE_IN_PROGRESS.store(false, Ordering::Relaxed);
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

fn start_meeting_poll(app: AppHandle) {
    // Prevent starting multiple poll loops
    if POLL_STARTED.swap(true, Ordering::Relaxed) {
        eprintln!("[Hush] Poll loop already running — skipping duplicate start");
        return;
    }
    std::thread::spawn(move || {
        // Enable DND quickly (2 polls × 3s = 6s) but disable slowly (10 polls × 3s = 30s).
        // The slow OFF threshold is intentional: when muted in Teams the mic signal
        // drops and the window title often isn't "meeting", so we get brief false
        // negatives mid-call. 30s of consecutive "no meeting" is required before
        // we decide the call actually ended.
        let mut consecutive_meeting = 0u32;
        let mut consecutive_no_meeting = 0u32;
        const DEBOUNCE_ON_COUNT: u32 = 2;   // 6s  — fast to enable DND
        const DEBOUNCE_OFF_COUNT: u32 = 10; // 30s — slow to disable DND

        let mut last_poll = std::time::Instant::now();

        loop {
            std::thread::sleep(std::time::Duration::from_secs(3));

            // Detect sleep/wake: if >30s passed, system was asleep.
            // Skip this cycle — state may be stale.
            let elapsed = last_poll.elapsed().as_secs();
            last_poll = std::time::Instant::now();
            if elapsed > 30 {
                eprintln!("[Hush] System wake detected ({}s gap) — resetting", elapsed);
                consecutive_meeting = 0;
                consecutive_no_meeting = 0;
                continue;
            }

            // Skip detection if user disabled auto-DND on screen share
            if !AUTO_DND_SCREEN_SHARE.load(Ordering::Relaxed) {
                consecutive_meeting = 0;
                consecutive_no_meeting = 0;
                continue;
            }

            let in_meeting = meeting::is_in_meeting();
            let hushed = IS_HUSHED.load(Ordering::Relaxed);
            let auto_hushed = AUTO_HUSHED_BY_MEETING.load(Ordering::Relaxed);

            if in_meeting {
                consecutive_meeting += 1;
                consecutive_no_meeting = 0;
            } else {
                consecutive_no_meeting += 1;
                consecutive_meeting = 0;
                // Screen sharing stopped — clear manual override so next
                // screen share session will auto-hush again
                if MANUAL_OVERRIDE.load(Ordering::Relaxed) && consecutive_no_meeting >= DEBOUNCE_OFF_COUNT {
                    MANUAL_OVERRIDE.store(false, Ordering::Relaxed);
                    eprintln!("[Hush] Manual override cleared — ready for next meeting");
                }
            }

            // Skip auto-hush if user manually overrode during this session
            if MANUAL_OVERRIDE.load(Ordering::Relaxed) {
                continue;
            }

            // Auto-hush ON: meeting detected for DEBOUNCE_ON_COUNT consecutive polls (6s)
            if in_meeting && !hushed && consecutive_meeting >= DEBOUNCE_ON_COUNT {
                eprintln!("[Hush] AUTO-HUSH ON — meeting detected for {}s", consecutive_meeting * 3);
                AUTO_HUSHED_BY_MEETING.store(true, Ordering::Relaxed);
                toggle_hush(&app, Some(true));
            }
            // Auto-hush OFF: meeting ended for DEBOUNCE_OFF_COUNT consecutive polls (30s)
            // AND we were the ones who turned DND on.
            // 30s threshold tolerates brief mic drops (mute) + window title misses mid-call.
            else if !in_meeting && hushed && auto_hushed && consecutive_no_meeting >= DEBOUNCE_OFF_COUNT {
                eprintln!("[Hush] AUTO-HUSH OFF — meeting ended for {}s", consecutive_no_meeting * 3);
                AUTO_HUSHED_BY_MEETING.store(false, Ordering::Relaxed);
                toggle_hush(&app, Some(false));
            }
        }
    });
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
    eprintln!("[Hush] Setup complete — starting meeting detection");
    start_meeting_poll(app);
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
            // Register the global menu event handler ONCE.
            // (Registering inside build_and_set_menu appended a new handler on
            // every rebuild, making each click fire multiple times.)
            app.on_menu_event(move |app_h, event: MenuEvent| match event.id().as_ref() {
                "toggle" => toggle_hush(app_h, None),
                "auto_screen_share" => {
                    let current = AUTO_DND_SCREEN_SHARE.load(Ordering::Relaxed);
                    AUTO_DND_SCREEN_SHARE.store(!current, Ordering::Relaxed);
                    eprintln!("[Hush] Auto-DND on Calls: {}", !current);
                }
                "play_sound" => {
                    let current = PLAY_SOUND.load(Ordering::Relaxed);
                    PLAY_SOUND.store(!current, Ordering::Relaxed);
                }
                "quit" => {
                    // If we turned DND on (auto or manual), turn it off before quitting
                    if IS_HUSHED.load(Ordering::Relaxed) {
                        let _ = dnd::set_dnd(false);
                    }
                    app_h.exit(0);
                }
                _ => {}
            });

            // Set up tray icon click handler
            if let Some(tray) = app.tray_by_id("hush-tray") {
                // Build and attach the menu so right-click shows it
                build_and_set_menu(app.handle());

                // Disable auto-showing menu on left click — we want left click to toggle DND
                let _ = tray.set_show_menu_on_left_click(false);

                tray.on_tray_icon_event(move |tray, event| {
                    match event {
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } => {
                            toggle_hush(tray.app_handle(), None);
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
                eprintln!("[Hush] Shortcuts found — starting meeting detection");
                // Hide setup window since shortcuts exist
                if let Some(win) = app.get_webview_window("setup") {
                    let _ = win.hide();
                }
                start_meeting_poll(app.handle().clone());
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, _event| {});
}

fn build_and_set_menu(app: &AppHandle) {
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
    let sep3 = PredefinedMenuItem::separator(app).unwrap();

    let auto_screen_share = CheckMenuItem::with_id(
        app,
        "auto_screen_share",
        "Auto-DND on Calls",
        true,
        AUTO_DND_SCREEN_SHARE.load(Ordering::Relaxed),
        None::<&str>,
    )
    .unwrap();

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
        &[&status, &sep1, &toggle, &sep2, &auto_screen_share, &sound, &sep3, &quit],
    )
    .unwrap();

    if let Some(tray) = app.tray_by_id("hush-tray") {
        let _ = tray.set_menu(Some(menu));
    }
}

