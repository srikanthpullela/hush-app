use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

static DETECTOR_BIN: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Swift detection script — compiled to a binary on first run for speed.
///
/// Three independent signals — any one is sufficient to consider "in a call":
///   1. Meeting app window title contains a call keyword
///   2. Meeting app has a floating call-controls bar onscreen (Teams/Zoom compact toolbar)
///   3. System microphone is active while a meeting app is running
///
/// Signal 2 is the most reliable for Teams: the floating bar (Camera/Mic/Share/Leave)
/// is ALWAYS present during a call, even when mic and camera are both muted/off.
/// Teams releases the mic device when the user mutes (for privacy), so signal 3
/// alone is not enough — we need the call bar as a fallback.
const DETECT_SCRIPT: &str = r#"
import CoreGraphics
import CoreAudio
import Foundation

let list = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] ?? []

let meetingApps = ["Microsoft Teams", "zoom.us", "Zoom", "Webex", "Cisco Webex",
                   "Slack", "FaceTime", "Google Meet"]

// Window title keywords indicating an active call/meeting.
// Broad intentionally — "Weekly Sync", "1:1 with John", "Standup" etc.
let callKeywords = ["meeting", "call", "standup", "stand-up", "sync", "webinar",
                    "conference", "interview", "1:1", "one-on-one", "hangout", "video chat"]

var hasMeetingWindow = false
var hasCallBar        = false   // floating compact call-controls window
var meetingAppRunning = false

for w in list {
    let owner    = w["kCGWindowOwnerName"] as? String ?? ""
    let name     = (w["kCGWindowName"] as? String ?? "").lowercased()
    let onscreen = w["kCGWindowIsOnscreen"] as? Bool ?? false
    let alpha    = (w["kCGWindowAlpha"] as? NSNumber)?.doubleValue ?? 0

    guard meetingApps.contains(where: { owner.contains($0) }) else { continue }
    meetingAppRunning = true

    // Signal 1: keyword in window title
    if callKeywords.contains(where: { name.contains($0) }) {
        hasMeetingWindow = true
    }

    // Signal 2: floating call-controls bar (Teams / Zoom compact toolbar).
    // When in a call, meeting apps show a narrow floating toolbar (sometimes
    // with a small self-view thumbnail attached):
    //   • onscreen and visible (alpha > 0)
    //   • height: 40–300 px  (a strip/compact panel, not a full window)
    //   • width: > 450 px    (wider than notification banners ~360 px)
    // This bar disappears the moment you leave the call.
    if onscreen && alpha > 0, let bounds = w["kCGWindowBounds"] as? [String: Any] {
        let h  = (bounds["Height"] as? NSNumber)?.doubleValue ?? 0
        let wd = (bounds["Width"]  as? NSNumber)?.doubleValue ?? 0
        if h > 40 && h < 300 && wd > 450 {
            hasCallBar = true
        }
    }
}

// Signal 3: default microphone in use (fails when user is muted in Teams)
var micInUse = false
if meetingAppRunning {
    var dev: AudioDeviceID = 0
    var sz = UInt32(MemoryLayout<AudioDeviceID>.size)
    var a1 = AudioObjectPropertyAddress(
        mSelector: kAudioHardwarePropertyDefaultInputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain
    )
    AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &a1, 0, nil, &sz, &dev)
    var running: UInt32 = 0
    sz = UInt32(MemoryLayout<UInt32>.size)
    var a2 = AudioObjectPropertyAddress(
        mSelector: kAudioDevicePropertyDeviceIsRunningSomewhere,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain
    )
    AudioObjectGetPropertyData(dev, &a2, 0, nil, &sz, &running)
    micInUse = running > 0
}

var signals: [String] = []
if hasMeetingWindow { signals.append("meeting-window") }
if hasCallBar       { signals.append("call-bar") }
if micInUse         { signals.append("mic-active") }

if meetingAppRunning && !signals.isEmpty {
    print("active:\(signals.joined(separator: ","))")
} else {
    print("none")
}
"#;

fn compile_detector() -> Option<PathBuf> {
    let dir = std::env::temp_dir().join("hush-app");
    let _ = std::fs::create_dir_all(&dir);
    let bin = dir.join("meeting-detect");
    let src = dir.join("meeting-detect.swift");

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
/// Returns true if a meeting app (Teams/Zoom/Webex/Slack/FaceTime) is running
/// AND either:
///   - A window title contains "meeting" (formal meeting/call)
///   - The system microphone is active (any type of call)
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
