#!/usr/bin/env swift
// Drives Lodestone Launcher (or Lodestone Host) on macOS through the accessibility
// API, in the background: it never moves the pointer, types on the keyboard or
// brings the app forward, so whoever is at the Mac can keep working.
//
// The launcher reports its controls through AccessKit, which gives each one its
// automation id as `AXIdentifier` (`play`, `account-box`, `instance-tabs-1`). Pressing
// a control calls its handler directly, as a screen reader would, instead of clicking
// on screen. `ui_drive.ps1` is the same tool for Windows (UI Automation).
//
//   swift launcher/scripts/ui_drive.swift tree
//   swift launcher/scripts/ui_drive.swift press settings
//   swift launcher/scripts/ui_drive.swift screenshot /tmp/launcher.png
//
// Or build it once, which starts faster: swiftc -O -o ui-drive launcher/scripts/ui_drive.swift
//
// Needs Accessibility permission for the app that runs it (Terminal, iTerm, the
// Claude app...) in System Settings > Privacy & Security > Accessibility, and Screen
// Recording permission there for `screenshot`. See launcher/docs/ui-automation.md.

import AppKit
import ApplicationServices
import Foundation

let usage = """
    Usage: ui_drive.swift [--app launcher|host|NAME] [--pid PID] [--json] COMMAND [ARGS]

    Drives Lodestone Launcher through the accessibility API without taking the
    pointer, the keyboard or the foreground.

    Commands:
      apps                  the running apps --app could mean, with their pids
      tree [--depth N]      every control: role, automation id, name, value, actions
      find ID               one control, by automation id
      press ID              clicks it (buttons, tabs, toggles, menu items, links)
      select ID             selects a tab or list item (Pick, else Press)
      set ID VALUE          sets a text input's text, or a slider's number
      value ID              prints its value
      focus ID              gives it keyboard focus
      increment ID          steps a slider up
      decrement ID          steps a slider down
      wait ID [--timeout S] waits until it exists (default 10 seconds)
      gone ID [--timeout S] waits until it no longer exists
      screenshot FILE.png   captures the app's window, even behind other windows

    --app picks the app: `launcher` (the default) or `host`, or any app's name.
    --pid picks one process, when several match (an installed and a dev launcher).
    --json prints `tree`, `find` and `apps` as JSON.

    Exit codes: 0 done, 1 failed, 2 no permission, 3 app not running,
    4 no such control, 64 bad usage.
    """

// MARK: - Output

func fail(_ message: String, code: Int32 = 1) -> Never {
    FileHandle.standardError.write(("ui_drive: " + message + "\n").data(using: .utf8)!)
    exit(code)
}

func printJSON(_ value: Any) {
    guard JSONSerialization.isValidJSONObject(value),
        let data = try? JSONSerialization.data(
            withJSONObject: value, options: [.prettyPrinted, .sortedKeys])
    else { fail("couldn't write JSON") }
    print(String(decoding: data, as: UTF8.self))
}

// MARK: - Arguments

var appName = "launcher"
var pidArg: pid_t?
var json = false
var depthLimit = 64
var timeout: Double = 10
var positional: [String] = []

var argv = Array(CommandLine.arguments.dropFirst())
while !argv.isEmpty {
    let arg = argv.removeFirst()
    func next() -> String {
        guard !argv.isEmpty else { fail("\(arg) needs a value\n\n\(usage)", code: 64) }
        return argv.removeFirst()
    }
    switch arg {
    case "-h", "--help", "help":
        print(usage)
        exit(0)
    case "--app": appName = next()
    case "--pid":
        guard let pid = pid_t(next()) else { fail("--pid takes a number", code: 64) }
        pidArg = pid
    case "--json": json = true
    case "--depth":
        guard let depth = Int(next()) else { fail("--depth takes a number", code: 64) }
        depthLimit = depth
    case "--timeout":
        guard let seconds = Double(next()) else { fail("--timeout takes seconds", code: 64) }
        timeout = seconds
    default: positional.append(arg)
    }
}

guard let command = positional.first else {
    print(usage)
    exit(64)
}
let commandArgs = Array(positional.dropFirst())

func argument(_ index: Int, _ name: String) -> String {
    guard index < commandArgs.count else { fail("\(command) needs \(name)\n\n\(usage)", code: 64) }
    return commandArgs[index]
}

// MARK: - Finding the app

/// The names an app goes by: the bundle's name, and the executable's for a dev build
/// started from `target/`.
func names(_ app: NSRunningApplication) -> [String] {
    [app.localizedName, app.executableURL?.lastPathComponent, app.bundleIdentifier]
        .compactMap { $0?.lowercased() }
}

func candidates() -> [NSRunningApplication] {
    let wanted: [String]
    switch appName.lowercased() {
    case "launcher": wanted = ["lodestone launcher", "lodestone-launcher"]
    case "host": wanted = ["lodestone host", "lodestone-host", "lodestone host dev"]
    default: wanted = [appName.lowercased()]
    }
    return NSWorkspace.shared.runningApplications.filter { app in
        !names(app).filter { wanted.contains($0) }.isEmpty
    }
}

func describe(_ app: NSRunningApplication) -> [String: Any] {
    [
        "pid": Int(app.processIdentifier),
        "name": app.localizedName ?? "",
        "path": app.executableURL?.path ?? "",
        "bundle_id": app.bundleIdentifier ?? "",
        "active": app.isActive,
    ]
}

func targetPid() -> pid_t {
    if let pid = pidArg {
        guard NSRunningApplication(processIdentifier: pid) != nil else {
            fail("no app with pid \(pid) is running", code: 3)
        }
        return pid
    }
    let apps = candidates()
    if apps.isEmpty {
        fail("\(appName) isn't running (no app named so). Start it, or pass --pid.", code: 3)
    }
    if apps.count > 1 {
        let list = apps.map { "  \($0.processIdentifier)  \($0.executableURL?.path ?? "")" }
            .joined(separator: "\n")
        fail("several apps match \(appName); pass --pid:\n\(list)")
    }
    return apps[0].processIdentifier
}

// MARK: - The accessibility tree

func ensureTrusted() {
    let prompt = kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
    if !AXIsProcessTrustedWithOptions([prompt: true] as CFDictionary) {
        fail(
            """
            this app has no Accessibility permission. Allow the app running this script \
            (Terminal, iTerm, the Claude app...) in System Settings > Privacy & Security > \
            Accessibility, then run it again.
            """, code: 2)
    }
}

func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else {
        return nil
    }
    return value
}

func text(_ element: AXUIElement, _ name: String) -> String? {
    guard let value = attribute(element, name) else { return nil }
    if let string = value as? String { return string.isEmpty ? nil : string }
    if let number = value as? NSNumber { return number.stringValue }
    return nil
}

func flag(_ element: AXUIElement, _ name: String) -> Bool? {
    (attribute(element, name) as? NSNumber)?.boolValue
}

func children(_ element: AXUIElement) -> [AXUIElement] {
    (attribute(element, kAXChildrenAttribute) as? [AXUIElement]) ?? []
}

func actions(_ element: AXUIElement) -> [String] {
    var names: CFArray?
    guard AXUIElementCopyActionNames(element, &names) == .success,
        let list = names as? [String]
    else { return [] }
    return list
}

func identifier(_ element: AXUIElement) -> String? { text(element, "AXIdentifier") }

/// What a control is called: AccessKit puts a button's label in its title and other
/// controls' in their description.
func label(_ element: AXUIElement) -> String? {
    text(element, kAXTitleAttribute) ?? text(element, kAXDescriptionAttribute)
        ?? text(element, kAXHelpAttribute)
}

struct Found {
    let element: AXUIElement
    let depth: Int
}

/// The app's windows and everything in them, depth first.
func walk(_ pid: pid_t) -> [Found] {
    let app = AXUIElementCreateApplication(pid)
    AXUIElementSetMessagingTimeout(app, 5)
    var found: [Found] = []
    func visit(_ element: AXUIElement, _ depth: Int) {
        found.append(Found(element: element, depth: depth))
        guard depth < depthLimit else { return }
        for child in children(element) { visit(child, depth + 1) }
    }
    let windows = (attribute(app, kAXWindowsAttribute) as? [AXUIElement]) ?? []
    for window in windows { visit(window, 0) }
    return found
}

func info(_ element: AXUIElement) -> [String: Any] {
    var result: [String: Any] = ["role": text(element, kAXRoleAttribute) ?? ""]
    if let subrole = text(element, kAXSubroleAttribute) { result["subrole"] = subrole }
    if let id = identifier(element) { result["id"] = id }
    if let name = label(element) { result["name"] = name }
    if let value = text(element, kAXValueAttribute) { result["value"] = value }
    if let enabled = flag(element, kAXEnabledAttribute), !enabled { result["enabled"] = false }
    if let focused = flag(element, kAXFocusedAttribute), focused { result["focused"] = true }
    if let selected = flag(element, kAXSelectedAttribute), selected { result["selected"] = true }
    let names = actions(element)
    if !names.isEmpty { result["actions"] = names }
    return result
}

func line(_ element: AXUIElement) -> String {
    let fields = info(element)
    var parts = [fields["role"] as? String ?? "?"]
    if let subrole = fields["subrole"] as? String { parts[0] += "/" + subrole }
    if let id = fields["id"] as? String { parts.append("#" + id) }
    if let name = fields["name"] as? String { parts.append("\"\(name)\"") }
    if let value = fields["value"] as? String { parts.append("value=\"\(value)\"") }
    if fields["enabled"] != nil { parts.append("disabled") }
    if fields["focused"] != nil { parts.append("focused") }
    if fields["selected"] != nil { parts.append("selected") }
    if let names = fields["actions"] as? [String] {
        parts.append("[" + names.map { $0.replacingOccurrences(of: "AX", with: "") }
            .joined(separator: " ") + "]")
    }
    return parts.joined(separator: " ")
}

/// The control with this automation id. The launcher builds its tree only once an
/// assistive app first asks, so the first look can come back empty: it asks again
/// for a moment before giving up.
func lookup(_ pid: pid_t, _ id: String, within seconds: Double = 2) -> AXUIElement? {
    let deadline = Date().addingTimeInterval(seconds)
    repeat {
        if let hit = walk(pid).first(where: { identifier($0.element) == id }) {
            return hit.element
        }
        Thread.sleep(forTimeInterval: 0.2)
    } while Date() < deadline
    return nil
}

func control(_ pid: pid_t, _ id: String) -> AXUIElement {
    guard let element = lookup(pid, id) else {
        fail("no control has the automation id \"\(id)\" (see `tree`)", code: 4)
    }
    return element
}

func check(_ error: AXError, _ what: String) {
    if error != .success {
        fail("\(what) failed (AXError \(error.rawValue)): the control may not support it")
    }
}

// MARK: - Screenshots

/// The app's main window in the window server's list: the largest on the normal layer.
func windowNumber(_ pid: pid_t) -> CGWindowID? {
    guard let list = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]]
    else { return nil }
    let windows = list.filter {
        ($0[kCGWindowOwnerPID as String] as? Int) == Int(pid)
            && ($0[kCGWindowLayer as String] as? Int) == 0
    }
    func area(_ window: [String: Any]) -> Double {
        guard let bounds = window[kCGWindowBounds as String] as? [String: Any],
            let width = bounds["Width"] as? Double, let height = bounds["Height"] as? Double
        else { return 0 }
        return width * height
    }
    guard let main = windows.max(by: { area($0) < area($1) }),
        let number = main[kCGWindowNumber as String] as? Int
    else { return nil }
    return CGWindowID(number)
}

func screenshot(_ pid: pid_t, to path: String) {
    guard let window = windowNumber(pid) else {
        fail("the app has no window to capture (is it minimized or still starting?)")
    }
    let capture = Process()
    capture.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
    // -l: that window only, as drawn even when covered; -o: no shadow; -x: no sound.
    capture.arguments = ["-x", "-o", "-l", String(window), path]
    do { try capture.run() } catch { fail("couldn't run screencapture: \(error)") }
    capture.waitUntilExit()
    guard capture.terminationStatus == 0, FileManager.default.fileExists(atPath: path) else {
        fail(
            """
            screencapture failed. Allow the app running this script in System Settings > \
            Privacy & Security > Screen Recording.
            """)
    }
    if !CGPreflightScreenCaptureAccess() {
        FileHandle.standardError.write(
            "ui_drive: no Screen Recording permission: the picture may show only the desktop.\n"
                .data(using: .utf8)!)
    }
    print(path)
}

// MARK: - Commands

switch command {
case "apps":
    let apps = candidates().map(describe)
    if json {
        printJSON(apps)
    } else if apps.isEmpty {
        fail("no app matches \(appName)", code: 3)
    } else {
        for app in apps { print("\(app["pid"]!)  \(app["name"]!)  \(app["path"]!)") }
    }

case "tree":
    ensureTrusted()
    let pid = targetPid()
    var nodes = walk(pid)
    // The first look asks the launcher to build its tree; look again once it has.
    if nodes.allSatisfy({ identifier($0.element) == nil }) {
        Thread.sleep(forTimeInterval: 0.5)
        nodes = walk(pid)
    }
    if json {
        printJSON(nodes.map { node -> [String: Any] in
            var fields = info(node.element)
            fields["depth"] = node.depth
            return fields
        })
    } else {
        for node in nodes {
            print(String(repeating: "  ", count: node.depth) + line(node.element))
        }
    }

case "find":
    ensureTrusted()
    let element = control(targetPid(), argument(0, "an automation id"))
    if json { printJSON(info(element)) } else { print(line(element)) }

case "press":
    ensureTrusted()
    let id = argument(0, "an automation id")
    check(AXUIElementPerformAction(control(targetPid(), id), kAXPressAction as CFString), "press \(id)")

case "select":
    ensureTrusted()
    let id = argument(0, "an automation id")
    let element = control(targetPid(), id)
    // A list item picks; a tab (a radio button to AppKit) presses.
    if AXUIElementPerformAction(element, kAXPickAction as CFString) != .success {
        check(AXUIElementPerformAction(element, kAXPressAction as CFString), "select \(id)")
    }

case "set":
    ensureTrusted()
    let id = argument(0, "an automation id")
    let value = argument(1, "a value")
    let element = control(targetPid(), id)
    let role = text(element, kAXRoleAttribute)
    // A slider takes a number, anything else the text.
    let typed: CFTypeRef
    if role == kAXSliderRole, let number = Double(value) {
        typed = NSNumber(value: number)
    } else {
        typed = value as CFString
    }
    check(AXUIElementSetAttributeValue(element, kAXValueAttribute as CFString, typed), "set \(id)")

case "value":
    ensureTrusted()
    let element = control(targetPid(), argument(0, "an automation id"))
    print(text(element, kAXValueAttribute) ?? "")

case "focus":
    ensureTrusted()
    let id = argument(0, "an automation id")
    check(
        AXUIElementSetAttributeValue(
            control(targetPid(), id), kAXFocusedAttribute as CFString, kCFBooleanTrue),
        "focus \(id)")

case "increment", "decrement":
    ensureTrusted()
    let id = argument(0, "an automation id")
    let action = command == "increment" ? kAXIncrementAction : kAXDecrementAction
    check(AXUIElementPerformAction(control(targetPid(), id), action as CFString), "\(command) \(id)")

case "wait":
    ensureTrusted()
    let id = argument(0, "an automation id")
    guard let element = lookup(targetPid(), id, within: timeout) else {
        fail("\"\(id)\" didn't appear within \(timeout) seconds", code: 4)
    }
    print(line(element))

case "gone":
    ensureTrusted()
    let id = argument(0, "an automation id")
    let pid = targetPid()
    let deadline = Date().addingTimeInterval(timeout)
    while walk(pid).contains(where: { identifier($0.element) == id }) {
        if Date() >= deadline { fail("\"\(id)\" is still there after \(timeout) seconds") }
        Thread.sleep(forTimeInterval: 0.2)
    }

case "screenshot":
    screenshot(targetPid(), to: argument(0, "a file name"))

default:
    fail("unknown command \(command)\n\n\(usage)", code: 64)
}
