use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

static DETECTOR_BIN: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Swift detection script — compiled to a binary on first run for speed.
///
/// Detection: meeting app window title contains "meeting" → in a meeting/call.
///
/// NOT used:
///   - Microphone (CoreAudio) — false positives from voice-input apps
///     like VoiceSync when Teams/Zoom run in background.
///   - Layer-3 overlay windows — stale on macOS 26, persist after meetings.
const DETECT_SCRIPT: &str = r#"
import CoreGraphics
import Foundation

let list = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] ?? []
let meetingApps = ["Microsoft Teams", "zoom.us", "Zoom", "Webex", "Cisco Webex", "Slack", "FaceTime"]
var hasMeetingWindow = false

for w in list {
    let owner = w["kCGWindowOwnerName"] as? String ?? ""
    let name = (w["kCGWindowName"] as? String ?? "").lowercased()
    let isMeetingApp = meetingApps.contains(where: { owner.contains($0) })
    guard isMeetingApp else { continue }
    if name.contains("meeting") { hasMeetingWindow = true; break }
}

if hasMeetingWindow {
    print("active:meeting-window")
} else {
    print("none")
}
"#;

fn compile_detector() -> Option<PathBuf> {
    let dir = std::env::temp_dir().join("hush-app");
    let _ = std::fs::create_dir_all(&dir);
    let bin = dir.join("meeting-detect");
    let src = dir.join("meeting-detect.swift");

    // Always recompile — ensures binary matches current script after updates.
    // OnceLock guarantees this only runs once per app launch.
    let _ = std::fs::remove_file(&bin);

    eprintln!("[Hush] Compiling meeting detector...");
    if std::fs::write(&src, DETECT_SCRIPT).is_err() {
        eprintln!("[Hush] Failed to write detector source");
        return None;
    }

    match Command::new("/usr/bin/swiftc")
        .args(["-O", "-o"])
        .arg(&bin)
        .arg(&src)
        .output()
    {
        Ok(out) if out.status.success() => {
            eprintln!("[Hush] Meeting detector compiled OK");
            Some(bin)
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            eprintln!("[Hush] Swift compile error: {err}");
            None
        }
        Err(e) => {
            eprintln!("[Hush] swiftc not available: {e}");
            None
        }
    }
}

fn get_detector() -> Option<&'static PathBuf> {
    DETECTOR_BIN.get_or_init(compile_detector).as_ref()
}

/// Check if user is in an active meeting or call.
///
/// Returns true if a meeting app (Teams/Zoom/Webex/Slack/FaceTime) has
/// a window whose title contains "meeting".
pub fn is_in_meeting() -> bool {
    #[cfg(target_os = "macos")]
    {
        let output = if let Some(bin) = get_detector() {
            Command::new(bin).output()
        } else {
            // Fallback: interpret Swift directly (slower, ~2-3s)
            Command::new("/usr/bin/swift")
                .arg("-e")
                .arg(DETECT_SCRIPT)
                .output()
        };

        let result = output
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|e| {
                eprintln!("[Hush] detection error: {e}");
                "error".to_string()
            });

        if result.starts_with("active:") {
            let signals = &result[7..];
            eprintln!("[Hush] detected: in meeting ({signals})");
            return true;
        }
        return false;
    }

    #[cfg(target_os = "windows")]
    {
        // TODO: Windows meeting detection
        return false;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    false
}
