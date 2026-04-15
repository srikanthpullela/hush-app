use std::process::Command;

/// Detect if user is actively sharing their screen in a meeting.
///
/// This is the ONLY auto-detection signal. We intentionally do NOT check:
///   - Mic/camera (unreliable — stale after sleep, false positives from
///     Voice Memos, Siri, dictation, etc.)
///   - UDP connections (meeting apps keep background connections for chat)
///   - Meeting app running alone (Teams/Slack always run in background)
///
/// CptHost is the macOS screen sharing host process — it ONLY runs during
/// an active screen share session in a meeting app. No false positives.
///
/// For calls without screen sharing, the user uses the manual tray toggle.
pub fn is_in_meeting() -> bool {
    let screen_sharing = check_screen_sharing();

    if screen_sharing {
        eprintln!("[Hush] meeting detected: screen sharing active");
    }

    screen_sharing
}

/// Check for screen sharing processes.
/// Only checks for CptHost — the macOS content sharing host process that
/// meeting apps (Zoom, Teams, Webex) spawn during active screen shares.
/// ScreenSharingAgent is excluded — it can run for macOS remote desktop
/// sessions unrelated to meetings.
fn check_screen_sharing() -> bool {
    #[cfg(target_os = "macos")]
    {
        return Command::new("/usr/bin/pgrep")
            .args(["-f", "CptHost"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    }

    #[cfg(target_os = "windows")]
    {
        let script = "tasklist /FI \"IMAGENAME eq dwmcore.dll\" 2>$null | Select-String screen";
        return Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    false
}
