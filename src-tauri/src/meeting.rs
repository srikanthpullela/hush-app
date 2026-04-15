use std::process::Command;

/// Detect if user is actively sharing their screen in a meeting app.
///
/// This is the ONLY auto-detection signal. We intentionally do NOT check:
///   - Mic/camera (unreliable — stale after sleep, false positives)
///   - UDP connections (meeting apps keep background connections for chat)
///   - Meeting app running alone (Teams/Slack always run in background)
///   - CptHost process (doesn't exist on macOS 26+)
///
/// Detection method: Meeting apps (Teams, Zoom, Webex) create a special
/// overlay window at CGWindowLayer 3 during active screen sharing. This
/// window ONLY exists during screen share and disappears when sharing stops.
///
/// For calls without screen sharing, the user uses the manual tray toggle.
pub fn is_in_meeting() -> bool {
    let screen_sharing = check_screen_sharing();

    if screen_sharing {
        eprintln!("[Hush] meeting detected: screen sharing active (overlay window found)");
    }

    screen_sharing
}

/// Check for active screen sharing by looking for meeting app overlay windows.
///
/// On macOS, meeting apps create a window at layer 3 during screen sharing.
/// This is the red/blue border overlay that appears around the shared screen.
/// We use a Swift script to query CGWindowListCopyWindowInfo for this signal.
fn check_screen_sharing() -> bool {
    #[cfg(target_os = "macos")]
    {
        // Check for meeting app windows at layer 3 (screen sharing overlay)
        let script = r#"
import CoreGraphics
import Foundation
let list = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] ?? []
let meetingApps = ["Microsoft Teams", "zoom.us", "Zoom", "Webex", "Cisco Webex", "Slack"]
var found = false
for w in list {
    let owner = w["kCGWindowOwnerName"] as? String ?? ""
    let layer = w["kCGWindowLayer"] as? Int ?? 0
    let onscreen = w["kCGWindowIsOnscreen"] as? Bool ?? false
    if layer == 3 && onscreen && meetingApps.contains(where: { owner.contains($0) }) {
        found = true
        break
    }
}
print(found ? "1" : "0")
"#;
        return Command::new("/usr/bin/swift")
            .arg("-e")
            .arg(script)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "1")
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
