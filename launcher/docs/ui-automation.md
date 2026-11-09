# Driving the launcher without taking over the desktop

Computer-use tools test an app the way a person would: they take screenshots of the
whole screen and move the real pointer and keyboard, so whoever is at the machine has to
stop working while they run. Lodestone Launcher and Lodestone Host don't need that. Their
controls are exposed to the platform's accessibility API, so a script can find a control
by its automation id, press it, fill it in and take a picture of the window, all in the
background, without moving the pointer or bringing the window forward.

| | macOS | Windows |
| --- | --- | --- |
| Tool | `launcher/scripts/ui_drive.swift` | `launcher/scripts/ui_drive.ps1` |
| API | Accessibility (`AXUIElement`) | UI Automation |
| Automation id is | `AXIdentifier` | `AutomationId` |
| Screenshot | `screencapture -l <window>` | `PrintWindow` (full content) |
| Permission | Accessibility, plus Screen Recording for screenshots | none |

The two tools take the same commands and print the same kind of output, so a test
written against one reads the same on the other.

## How it works

The shared components in `lodestone-ui` give every control a role, a label and an
automation id (`components/a11y.rs`); GPUI hands them to AccessKit, which serves them to
UI Automation on Windows, the accessibility API on macOS and AT-SPI on Linux. The
automation id is the control's element id, the same in every language and release:
`play`, `settings`, `account-box`, `instance-tabs-1`, a list item's id plus its own.

Pressing a control sends AccessKit's Click to the control's own handler (`on_press`), as a
screen reader would, so it works on a control scrolled out of view or behind another
window. GPUI builds the tree only once an assistive app first asks for it, so the tools
look again for a moment when the first look comes back empty.

## Setup

### macOS

1. Swift comes with the Xcode command line tools (`xcode-select --install`).
2. In System Settings > Privacy & Security > **Accessibility**, allow the app that runs
   the script: Terminal, iTerm, the Claude app or your editor. The first run asks for
   it. Without it the tool stops with exit code 2.
3. For `screenshot`, allow the same app in **Screen Recording** too. Without it the
   picture shows only the desktop behind the window.

Run it with `swift launcher/scripts/ui_drive.swift COMMAND`, or build it once, which
starts faster:

```sh
swiftc -O -o /tmp/ui-drive launcher/scripts/ui_drive.swift
/tmp/ui-drive tree
```

### Windows

Nothing to set up. Run it with PowerShell 7 or Windows PowerShell:

```powershell
pwsh launcher/scripts/ui_drive.ps1 tree
```

## Commands

```
ui_drive [--app launcher|host|NAME] [--pid PID] [--json] COMMAND [ARGS]

apps                  the running apps --app could mean, with their pids
tree [--depth N]      every control: role, automation id, name, value, actions
find ID               one control, by automation id
press ID              clicks it (buttons, tabs, toggles, menu buttons, links)
select ID             selects a tab or list item
set ID VALUE          sets a text input's text, or a slider's number
value ID              prints its value
focus ID              gives it keyboard focus
increment ID          steps a slider up
decrement ID          steps a slider down
wait ID [--timeout S] waits until it exists (default 10 seconds)
gone ID [--timeout S] waits until it no longer exists
screenshot FILE.png   captures the app's window, even behind other windows
```

Exit codes: 0 done, 1 failed, 2 no Accessibility permission (macOS), 3 app not running,
4 no such control, 64 bad usage.

`--app` defaults to the launcher; `--app host` drives Lodestone Host. When an installed
launcher and a dev build both run, `apps` lists them and `--pid` picks one.

A session, on either platform:

```sh
ui_drive tree                         # what's on screen, with ids
ui_drive press settings               # open Settings
ui_drive wait settings-page-title     # wait for the page
ui_drive screenshot /tmp/settings.png # look at it
ui_drive press account-box            # open the account menu
ui_drive press add-offline            # its "Add offline account" item
ui_drive set offline-username Steve   # fill in the dialog's field
```

`tree` prints one control per line, indented by depth:

```
Window "Lodestone Launcher"
  Group #sidebar
    Button #account-box "ShironaYuki" collapsed [ExpandCollapse]
    Button #settings "Settings" [Invoke]
  Tab #instance-tabs
    TabItem #instance-tabs-0 "Play" selected [SelectionItem]
```

Role names are the platform's own (`AXButton` on macOS, `Button` on Windows); ids,
labels and values are the same.

## Limits

- **Dialog text isn't in the tree**, only a dialog's buttons and inputs. Read results
  from files (instance JSON, logs, crash reports), the
  [command line](../crates/lodestone-launcher-cli/README.md), or a screenshot.
- **Some actions bring the window forward on purpose**: opening an invite link,
  bringing the launcher back after a game crashes, opening the skin editor or the
  performance monitor (`activate_window`). A native file picker opens in front too.
- **`focus` on Windows** can bring the window forward (UI Automation's `SetFocus`).
  `press` and `select` don't.
- **A minimized window can't be captured** on either platform.
- **The New instance version list** isn't reachable yet; make the instance with the
  command line (`lodestone-launcher-cli instances create`) instead.
- **Debug builds panic on a duplicate automation id** once a client walks the tree
  (GPUI asserts on it; release builds drop the node). That makes the tools a cheap
  check for duplicate ids: walk every screen of a debug build and watch its output for
  "Duplicate a11y node id".

## Testing without touching your own launcher

A launcher started with `LODESTONE_LAUNCHER_DATA_DIR` set to a scratch folder has its
own instances, accounts and settings. On Windows a second launcher hands off to the one
already running and quits, unless it was built with `--features screenshot`, which skips
that check. For pictures of a prepared state without any window at all, the
`screenshot` feature's `--scene NAME --screenshot FILE.png` renders one offscreen
(`crates/lodestone-launcher/src/preview.rs` lists the scenes).
