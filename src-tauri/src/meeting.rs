use std::process::Command;

/// Detect if user is currently in a meeting.
/// Triggers ONLY when:
///   1. A known meeting app is running AND screen sharing is active, OR
///   2. A known meeting app is running AND mic/camera is actively in use
///
/// A meeting app MUST be running for any auto-detection to trigger.
/// Manual toggle via tray icon always works regardless.
pub fn is_in_meeting() -> bool {
    let meeting_app = is_meeting_app_running();
    if !meeting_app {
        return false;
    }

    let screen_sharing = check_screen_sharing();
    let mic_camera = check_mic_camera();

    let result = screen_sharing || mic_camera;

    eprintln!(
        "[Hush] meeting check: meeting_app={}, screen_sharing={}, mic_camera={} → {}",
        meeting_app, screen_sharing, mic_camera, result
    );

    result
}

/// Check if a known meeting app (Teams, Zoom, Webex, Slack, Google Meet via browser) is running.
fn is_meeting_app_running() -> bool {
    #[cfg(target_os = "macos")]
    {
        for proc in &["MSTeams", "zoom.us", "Webex", "Slack", "FaceTime"] {
            if Command::new("/usr/bin/pgrep")
                .args(["-x", proc])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return true;
            }
        }
        return false;
    }

    #[cfg(target_os = "windows")]
    {
        let apps = ["Teams", "Zoom", "Webex", "Slack"];
        for app in &apps {
            let script = format!(
                "Get-Process -Name '*{}*' -ErrorAction SilentlyContinue | Select-Object -First 1",
                app
            );
            if Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .output()
                .map(|o| !o.stdout.is_empty())
                .unwrap_or(false)
            {
                return true;
            }
        }
        return false;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    false
}

/// Check if mic or camera is in use by another app.
fn check_mic_camera() -> bool {
    #[cfg(target_os = "macos")]
    {
        let script = r#"
import AVFoundation
var active = false
if let mic = AVCaptureDevice.default(for: .audio), mic.isInUseByAnotherApplication { active = true }
if let cam = AVCaptureDevice.default(for: .video), cam.isInUseByAnotherApplication { active = true }
print(active ? "1" : "0")
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
        // Check if any audio capture device is active via PowerShell
        let script = r#"
$mic = Get-PnpDevice -Class AudioEndpoint -Status OK -ErrorAction SilentlyContinue |
  Where-Object { $_.FriendlyName -match 'Microphone' }
if ($mic) { Write-Output '1' } else { Write-Output '0' }
"#;
        return Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "1")
            .unwrap_or(false);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    false
}

/// Check for screen sharing processes.
fn check_screen_sharing() -> bool {
    #[cfg(target_os = "macos")]
    {
        // Note: "screencaptureui" is excluded — it's the macOS screenshot tool (Cmd+Shift+4),
        // not actual screen sharing. Including it would falsely trigger DND on screenshots.
        for proc in &["CptHost", "ScreenSharingAgent"] {
            if Command::new("/usr/bin/pgrep")
                .args(["-f", proc])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return true;
            }
        }
        return false;
    }

    #[cfg(target_os = "windows")]
    {
        // Check for common screen sharing indicators
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

/// Check if meeting apps (Teams, Zoom, Webex) have active UDP media streams.
fn check_meeting_connections() -> bool {
    #[cfg(target_os = "macos")]
    {
        let apps = [
            ("MSTeams", "MSTeams"),
            ("zoom.us", "zoom"),
            ("Webex", "Webex"),
        ];
        for (pgrep_name, lsof_name) in &apps {
            let running = Command::new("/usr/bin/pgrep")
                .args(["-x", pgrep_name])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if running {
                let udp = Command::new("/usr/sbin/lsof")
                    .args(["-a", "-i", "UDP", "-c", lsof_name])
                    .output()
                    .ok();
                if let Some(o) = udp {
                    let output = String::from_utf8_lossy(&o.stdout).to_string();
                    let lines: Vec<_> = output
                        .lines()
                        .filter(|l| !l.is_empty())
                        .collect();
                    if lines.len() > 3 {
                        return true;
                    }
                }
            }
        }
        return false;
    }

    #[cfg(target_os = "windows")]
    {
        let apps = ["Teams", "Zoom", "Webex"];
        for app in &apps {
            let script = format!(
                "Get-Process -Name '*{}*' -ErrorAction SilentlyContinue | Select-Object -First 1",
                app
            );
            if Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .output()
                .map(|o| !o.stdout.is_empty())
                .unwrap_or(false)
            {
                return true;
            }
        }
        return false;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    false
}
