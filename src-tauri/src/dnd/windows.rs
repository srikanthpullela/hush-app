use std::process::Command;

/// Check if Focus Assist / DND is active on Windows.
pub fn is_dnd_active() -> bool {
    // Use PowerShell to query the Focus Assist state via WNF (Windows Notification Facility)
    let script = r#"
$path = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\CloudStore\Store\DefaultAccount\Current\default$windows.data.shellcommon.quietmomentfullscreen\windows.data.shellcommon.quietmomentfullscreen'
if (Test-Path $path) {
    $val = Get-ItemPropertyValue -Path $path -Name 'Data' -ErrorAction SilentlyContinue
    if ($val -and $val.Length -gt 15 -and $val[15] -ne 0) { Write-Output '1' } else { Write-Output '0' }
} else { Write-Output '0' }
"#;
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "1")
        .unwrap_or(false)
}

/// Toggle DND / Focus Assist on Windows via registry + settings broadcast.
pub fn set_dnd(on: bool) -> bool {
    // Windows 10/11: Toggle Focus Assist via Quiet Hours registry
    let script = if on {
        r#"
# Enable Focus Assist (Priority Only mode = 1, Alarms Only = 2)
$basePath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\CloudStore\Store\DefaultAccount\Current'
$qhPath = "$basePath\default`$windows.data.shellcommon.quiethourssettings\windows.data.shellcommon.quiethourssettings"
if (!(Test-Path $qhPath)) { New-Item -Path $qhPath -Force | Out-Null }

# Set to Alarms Only (most restrictive)
$data = @(0x02, 0x00, 0x00, 0x00) + @(0x00) * 8 + @(0x43, 0x42, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00)
Set-ItemProperty -Path $qhPath -Name 'Data' -Value ([byte[]]$data) -Type Binary -Force

# Broadcast settings change
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class WinApi {
    [DllImport("user32.dll", SetLastError = true)]
    public static extern IntPtr SendMessageTimeout(
        IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam,
        uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
    public static void BroadcastChange() {
        UIntPtr result;
        SendMessageTimeout((IntPtr)0xFFFF, 0x001A, UIntPtr.Zero, "Policy",
            0x0002, 1000, out result);
    }
}
"@
[WinApi]::BroadcastChange()
Write-Output 'OK'
"#
    } else {
        r#"
# Disable Focus Assist (back to normal)
$basePath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\CloudStore\Store\DefaultAccount\Current'
$qhPath = "$basePath\default`$windows.data.shellcommon.quiethourssettings\windows.data.shellcommon.quiethourssettings"
if (Test-Path $qhPath) {
    Remove-ItemProperty -Path $qhPath -Name 'Data' -ErrorAction SilentlyContinue
}

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class WinApi2 {
    [DllImport("user32.dll", SetLastError = true)]
    public static extern IntPtr SendMessageTimeout(
        IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam,
        uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
    public static void BroadcastChange() {
        UIntPtr result;
        SendMessageTimeout((IntPtr)0xFFFF, 0x001A, UIntPtr.Zero, "Policy",
            0x0002, 1000, out result);
    }
}
"@
[WinApi2]::BroadcastChange()
Write-Output 'OK'
"#
    };

    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("OK"))
        .unwrap_or(false)
}
