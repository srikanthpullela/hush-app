#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// Toggle DND on or off. Returns true on success.
pub fn set_dnd(on: bool) -> bool {
    #[cfg(target_os = "macos")]
    return macos::set_dnd(on);

    #[cfg(target_os = "windows")]
    return windows::set_dnd(on);

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = on;
        false
    }
}

/// Check which shortcuts exist. Returns (has_hush_on, has_hush_off).
pub fn check_shortcuts() -> (bool, bool) {
    #[cfg(target_os = "macos")]
    return macos::check_shortcuts();

    #[cfg(target_os = "windows")]
    return (true, true); // Windows doesn't need shortcuts

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return (false, false);
}

/// Try to auto-create shortcuts. Returns true if successful.
pub fn try_auto_create_shortcuts() -> bool {
    #[cfg(target_os = "macos")]
    return macos::try_auto_create_shortcuts();

    #[cfg(target_os = "windows")]
    return true;

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return false;
}

/// Open the Shortcuts app for manual creation.
pub fn open_shortcuts_app() {
    #[cfg(target_os = "macos")]
    macos::open_shortcuts_app();
}

/// Check if DND / Focus is currently active.
pub fn is_dnd_active() -> bool {
    #[cfg(target_os = "macos")]
    return macos::is_dnd_active();

    #[cfg(target_os = "windows")]
    return windows::is_dnd_active();

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    false
}
