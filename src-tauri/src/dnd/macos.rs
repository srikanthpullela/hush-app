use std::process::Command;
use std::sync::{Mutex, OnceLock};

// ── macOS version detection ─────────────────────────────

/// Cached macOS major version (e.g. 12 for Monterey, 14 for Sonoma).
static MACOS_MAJOR: OnceLock<u32> = OnceLock::new();

/// Cached actual shortcut names (resolved case-insensitively).
/// Wrapped in Mutex so they can be updated when user creates shortcuts during setup.
static SHORTCUT_ON_NAME: Mutex<Option<String>> = Mutex::new(None);
static SHORTCUT_OFF_NAME: Mutex<Option<String>> = Mutex::new(None);

fn macos_major_version() -> u32 {
    *MACOS_MAJOR.get_or_init(|| {
        Command::new("/usr/bin/sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|v| v.trim().split('.').next().map(String::from))
            .and_then(|s| s.parse().ok())
            .unwrap_or(12)
    })
}

/// Returns true if this Mac uses legacy Notification Center DND (pre-Monterey).
fn is_legacy_dnd() -> bool {
    macos_major_version() < 12
}

// ── Shortcut checks (macOS 12+ only) ───────────────────

/// Get the list of all shortcuts installed on this Mac.
fn get_shortcut_list() -> Vec<String> {
    Command::new("/usr/bin/shortcuts")
        .arg("list")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .lines()
        .map(|l| l.to_string())
        .collect()
}

/// Find the actual shortcut name matching the target (case-insensitive).
/// Returns the exact name as it appears in Shortcuts.app, or None.
fn find_shortcut_name(target: &str) -> Option<String> {
    get_shortcut_list()
        .into_iter()
        .find(|l| l.eq_ignore_ascii_case(target))
}

/// Resolve and cache the actual shortcut names.
fn resolve_shortcut_names() -> (bool, bool) {
    let on = find_shortcut_name("Hush On");
    let off = find_shortcut_name("Hush Off");
    let has_on = on.is_some();
    let has_off = off.is_some();

    if let Some(name) = &on {
        eprintln!("[Hush] Found 'Hush On' shortcut as: \"{}\"", name);
    }
    if let Some(name) = &off {
        eprintln!("[Hush] Found 'Hush Off' shortcut as: \"{}\"", name);
    }

    *SHORTCUT_ON_NAME.lock().unwrap() = on;
    *SHORTCUT_OFF_NAME.lock().unwrap() = off;

    (has_on, has_off)
}

/// Check which shortcuts exist. Returns (has_hush_on, has_hush_off).
/// On legacy macOS (<12), shortcuts aren't needed — returns (true, true).
/// Always does a fresh lookup and updates the cached names.
pub fn check_shortcuts() -> (bool, bool) {
    if is_legacy_dnd() {
        return (true, true);
    }

    resolve_shortcut_names()
}

/// Try to auto-create shortcuts.
/// On legacy macOS, always returns true (no shortcuts needed).
pub fn try_auto_create_shortcuts() -> bool {
    if is_legacy_dnd() {
        return true;
    }

    let on_result = Command::new("/usr/bin/shortcuts")
        .args(["run", "Hush On"])
        .output();
    let off_result = Command::new("/usr/bin/shortcuts")
        .args(["run", "Hush Off"])
        .output();

    let on_ok = on_result.map(|o| o.status.success()).unwrap_or(false);
    let off_ok = off_result.map(|o| o.status.success()).unwrap_or(false);
    on_ok && off_ok
}

/// Open the Shortcuts app for manual shortcut creation.
pub fn open_shortcuts_app() {
    let _ = Command::new("/usr/bin/open")
        .arg("-a")
        .arg("Shortcuts")
        .spawn();
}

// ── DND status ──────────────────────────────────────────

/// Check if DND / Focus is currently active.
pub fn is_dnd_active() -> bool {
    if is_legacy_dnd() {
        return Command::new("/usr/bin/defaults")
            .args(["-currentHost", "read", "com.apple.notificationcenterui", "doNotDisturb"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "1")
            .unwrap_or(false);
    }
    false
}

// ── DND toggle ──────────────────────────────────────────

/// Toggle DND — picks the right method for the macOS version:
///   macOS 12+:     Apple Shortcuts ("Hush On" / "Hush Off")
///   macOS 10.15–11: defaults write to Notification Center (completely silent)
pub fn set_dnd(on: bool) -> bool {
    if is_legacy_dnd() {
        return set_dnd_legacy(on);
    }
    set_dnd_shortcuts(on)
}

/// macOS 12+: run the named Shortcut.
/// Uses the cached actual shortcut name (resolved case-insensitively) so that
/// "hush on", "Hush On", "HUSH ON" all work.
fn set_dnd_shortcuts(on: bool) -> bool {
    let cached_name = if on {
        SHORTCUT_ON_NAME.lock().unwrap().clone()
    } else {
        SHORTCUT_OFF_NAME.lock().unwrap().clone()
    };

    let name = match cached_name {
        Some(n) => n,
        None => {
            // Cache miss — try to resolve now
            resolve_shortcut_names();
            let retry = if on {
                SHORTCUT_ON_NAME.lock().unwrap().clone()
            } else {
                SHORTCUT_OFF_NAME.lock().unwrap().clone()
            };
            match retry {
                Some(n) => n,
                None => {
                    eprintln!("[Hush] Shortcut not found for {}", if on { "Hush On" } else { "Hush Off" });
                    return false;
                }
            }
        }
    };

    eprintln!("[Hush] Running shortcut: \"{}\"", name);
    Command::new("/usr/bin/shortcuts")
        .args(["run", &name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// macOS 10.15–11: toggle DND via Notification Center defaults.
/// Completely silent — no Shortcuts, no banners, no user setup needed.
fn set_dnd_legacy(on: bool) -> bool {
    let bool_val = if on { "true" } else { "false" };

    let write_ok = Command::new("/usr/bin/defaults")
        .args([
            "-currentHost", "write",
            "com.apple.notificationcenterui",
            "doNotDisturb", "-bool", bool_val,
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if on {
        // Set the timestamp so macOS knows when DND was enabled
        let _ = Command::new("/bin/sh")
            .args([
                "-c",
                "defaults -currentHost write com.apple.notificationcenterui \
                 doNotDisturbDate -date \"$(date -u +'%Y-%m-%dT%H:%M:%SZ')\"",
            ])
            .output();
    }

    // Restart NotificationCenter to pick up the change
    let _ = Command::new("/usr/bin/killall")
        .arg("NotificationCenter")
        .output();

    write_ok
}
