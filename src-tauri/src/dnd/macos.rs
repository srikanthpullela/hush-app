use std::process::Command;
use std::sync::OnceLock;

// ── macOS version detection ─────────────────────────────

/// Cached macOS major version (e.g. 12 for Monterey, 14 for Sonoma).
static MACOS_MAJOR: OnceLock<u32> = OnceLock::new();

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

/// Check which shortcuts exist. Returns (has_hush_on, has_hush_off).
/// On legacy macOS (<12), shortcuts aren't needed — returns (true, true).
pub fn check_shortcuts() -> (bool, bool) {
    if is_legacy_dnd() {
        return (true, true);
    }

    let list = Command::new("/usr/bin/shortcuts")
        .arg("list")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();

    let has_on = list.lines().any(|l| l.eq_ignore_ascii_case("Hush On"));
    let has_off = list.lines().any(|l| l.eq_ignore_ascii_case("Hush Off"));
    (has_on, has_off)
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
fn set_dnd_shortcuts(on: bool) -> bool {
    let name = if on { "Hush On" } else { "Hush Off" };
    Command::new("/usr/bin/shortcuts")
        .args(["run", name])
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
