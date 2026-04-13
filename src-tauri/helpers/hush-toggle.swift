#!/usr/bin/env swift
// hush-toggle: Fast DND toggle via Accessibility API
// Usage: hush-toggle on|off|status
// Pre-compiled for speed — avoids `swift -e` compilation overhead.

import ApplicationServices
import Cocoa

// MARK: - AX Helpers

func controlCenterPid() -> pid_t? {
    NSRunningApplication.runningApplications(
        withBundleIdentifier: "com.apple.controlcenter"
    ).first?.processIdentifier
}

func findById(_ root: AXUIElement, id: String, depth: Int = 0) -> AXUIElement? {
    if depth > 15 { return nil }
    var idRef: CFTypeRef?
    AXUIElementCopyAttributeValue(root, kAXIdentifierAttribute as CFString, &idRef)
    if let i = idRef as? String, i == id { return root }
    for attr in [kAXChildrenAttribute, kAXWindowsAttribute] as [String] {
        var ch: CFTypeRef?
        AXUIElementCopyAttributeValue(root, attr as CFString, &ch)
        if let children = ch as? [AXUIElement] {
            for c in children {
                if let found = findById(c, id: id, depth: depth + 1) { return found }
            }
        }
    }
    return nil
}

/// Poll for an AX element by id, retrying up to `attempts` times with `intervalMs` between each.
func pollForElement(_ root: AXUIElement, id: String, attempts: Int = 15, intervalMs: UInt32 = 200) -> AXUIElement? {
    for _ in 0..<attempts {
        if let el = findById(root, id: id) { return el }
        usleep(intervalMs * 1000)
    }
    return nil
}

/// Send Escape key to dismiss panels
func dismiss() {
    let src = CGEventSource(stateID: .combinedSessionState)
    if let d = CGEvent(keyboardEventSource: src, virtualKey: 53, keyDown: true),
       let u = CGEvent(keyboardEventSource: src, virtualKey: 53, keyDown: false) {
        d.post(tap: .cghidEventTap)
        u.post(tap: .cghidEventTap)
    }
}

/// Click an AX element using CGEvent mouse click at its center position
func clickElement(_ el: AXUIElement) -> Bool {
    var posRef: CFTypeRef?
    var sizeRef: CFTypeRef?
    AXUIElementCopyAttributeValue(el, kAXPositionAttribute as CFString, &posRef)
    AXUIElementCopyAttributeValue(el, kAXSizeAttribute as CFString, &sizeRef)

    var pos = CGPoint.zero
    var size = CGSize.zero
    if let p = posRef { AXValueGetValue(p as! AXValue, .cgPoint, &pos) }
    if let s = sizeRef { AXValueGetValue(s as! AXValue, .cgSize, &size) }

    guard size.width > 0 && size.height > 0 else {
        fputs("WARN: Element has zero size\n", stderr)
        return false
    }

    let pt = CGPoint(x: pos.x + size.width / 2, y: pos.y + size.height / 2)
    let src = CGEventSource(stateID: .combinedSessionState)
    if let down = CGEvent(mouseEventSource: src, mouseType: .leftMouseDown, mouseCursorPosition: pt, mouseButton: .left),
       let up = CGEvent(mouseEventSource: src, mouseType: .leftMouseUp, mouseCursorPosition: pt, mouseButton: .left) {
        down.post(tap: .cghidEventTap)
        usleep(50_000)
        up.post(tap: .cghidEventTap)
        return true
    }
    return false
}

// MARK: - Status Check

func isFocusActive() -> Bool {
    guard let pid = controlCenterPid() else { return false }
    let app = AXUIElementCreateApplication(pid)
    return findById(app, id: "com.apple.menuextra.focusmode") != nil
}

// MARK: - Toggle DND

func toggleDND(on: Bool) -> Bool {
    guard let pid = controlCenterPid() else {
        fputs("ERROR: Control Center not found\n", stderr)
        return false
    }
    let app = AXUIElementCreateApplication(pid)

    let currentlyActive = isFocusActive()
    if on == currentlyActive {
        return true // Already in desired state
    }

    // Dismiss any open panel first
    dismiss()
    usleep(600_000)

    let dndId = "focus-mode-activity-com.apple.donotdisturb.mode.default"

    if on {
        // === TURNING ON: CC icon → Focus tile → submenu → DND checkbox ===
        guard let ccIcon = findById(app, id: "com.apple.menuextra.controlcenter") else {
            fputs("ERROR: CC icon not found\n", stderr)
            return false
        }

        // Try clicking CC icon up to 3 times (sometimes first click doesn't open panel)
        var focusTile: AXUIElement? = nil
        for attempt in 0..<3 {
            if attempt > 0 {
                dismiss()
                usleep(500_000)
            }
            let _ = clickElement(ccIcon)
            focusTile = pollForElement(app, id: "controlcenter-focus-modes", attempts: 8, intervalMs: 200)
            if focusTile != nil { break }
        }
        guard let focusTile = focusTile else {
            fputs("ERROR: Focus tile not found after retries\n", stderr)
            dismiss()
            return false
        }
        guard clickElement(focusTile) else {
            fputs("ERROR: Could not click focus tile\n", stderr)
            dismiss()
            return false
        }

        guard let dndItem = pollForElement(app, id: dndId) else {
            fputs("ERROR: DND checkbox not found\n", stderr)
            dismiss(); usleep(200_000); dismiss()
            return false
        }
        guard clickElement(dndItem) else {
            fputs("ERROR: Could not click DND checkbox\n", stderr)
            dismiss(); usleep(200_000); dismiss()
            return false
        }
    } else {
        // === TURNING OFF: Focus menu bar icon → DND checkbox (val=1) ===
        guard let focusIcon = findById(app, id: "com.apple.menuextra.focusmode") else {
            fputs("ERROR: Focus icon not found\n", stderr)
            return false
        }
        guard clickElement(focusIcon) else {
            fputs("ERROR: Could not click Focus icon\n", stderr)
            return false
        }

        // Poll for the DND checkbox (first one with this id will have val=1)
        guard let dndItem = pollForElement(app, id: dndId) else {
            fputs("ERROR: DND checkbox not found in Focus dropdown\n", stderr)
            dismiss()
            return false
        }
        guard clickElement(dndItem) else {
            fputs("ERROR: Could not click DND checkbox\n", stderr)
            dismiss()
            return false
        }
    }

    usleep(200_000)
    dismiss()
    usleep(200_000)
    dismiss()

    return true
}

// MARK: - Main

let args = CommandLine.arguments
guard args.count >= 2 else {
    print("Usage: hush-toggle on|off|status")
    exit(1)
}

switch args[1] {
case "on":
    exit(toggleDND(on: true) ? 0 : 1)
case "off":
    exit(toggleDND(on: false) ? 0 : 1)
case "status":
    print(isFocusActive() ? "on" : "off")
    exit(0)
default:
    print("Usage: hush-toggle on|off|status")
    exit(1)
}
