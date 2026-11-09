# Drives Lodestone Launcher (or Lodestone Host) on Windows through UI Automation, in
# the background: it never moves the pointer, types on the keyboard or brings the
# window forward, so whoever is at the PC can keep working.
#
# The launcher reports its controls through AccessKit, which gives each one its
# automation id as UIA's AutomationId (`play`, `account-box`, `instance-tabs-1`).
# Pressing a control calls its handler directly (Invoke, Toggle, Select), instead of
# clicking on screen. `ui_drive.swift` is the same tool for macOS.
#
#   pwsh launcher/scripts/ui_drive.ps1 tree
#   pwsh launcher/scripts/ui_drive.ps1 press settings
#   pwsh launcher/scripts/ui_drive.ps1 screenshot $env:TEMP\launcher.png
#
# See launcher/docs/ui-automation.md.

$ErrorActionPreference = 'Stop'

$usage = @'
Usage: ui_drive.ps1 [--app launcher|host|NAME] [--pid PID] [--json] COMMAND [ARGS]

Drives Lodestone Launcher through UI Automation without taking the pointer, the
keyboard or the foreground.

Commands:
  apps                  the running apps --app could mean, with their pids
  tree [--depth N]      every control: role, automation id, name, value, actions
  find ID               one control, by automation id
  press ID              clicks it (buttons, tabs, toggles, menu buttons, links)
  select ID             selects a tab or list item (Select, else Invoke)
  set ID VALUE          sets a text input's text, or a slider's number
  value ID              prints its value
  focus ID              gives it keyboard focus (Windows may bring the window forward)
  increment ID          steps a slider up
  decrement ID          steps a slider down
  wait ID [--timeout S] waits until it exists (default 10 seconds)
  gone ID [--timeout S] waits until it no longer exists
  screenshot FILE.png   captures the app's window, even behind other windows

--app picks the app: `launcher` (the default) or `host`, or any process name.
--pid picks one process, when several match (an installed and a dev launcher).
--json prints `tree`, `find` and `apps` as JSON.

Exit codes: 0 done, 1 failed, 3 app not running, 4 no such control, 64 bad usage.
'@

function Fail([string]$message, [int]$code = 1) {
    [Console]::Error.WriteLine("ui_drive: $message")
    exit $code
}

# --- Arguments ---

$appName = 'launcher'
$pidArg = $null
$json = $false
$depthLimit = 64
$timeout = 10.0
$positional = [System.Collections.Generic.List[string]]::new()

for ($i = 0; $i -lt $args.Count; $i++) {
    $arg = [string]$args[$i]
    $needsValue = $arg -in '--app', '--pid', '--depth', '--timeout'
    if ($needsValue) {
        if ($i + 1 -ge $args.Count) { Fail "$arg needs a value`n`n$usage" 64 }
        $value = [string]$args[++$i]
    }
    switch ($arg) {
        { $_ -in '-h', '--help', 'help' } { Write-Output $usage; exit 0 }
        '--app' { $appName = $value }
        '--pid' { if ($value -notmatch '^\d+$') { Fail '--pid takes a number' 64 }; $pidArg = [int]$value }
        '--json' { $json = $true }
        '--depth' { if ($value -notmatch '^\d+$') { Fail '--depth takes a number' 64 }; $depthLimit = [int]$value }
        '--timeout' {
            $seconds = 0.0
            if (-not [double]::TryParse($value, [ref]$seconds)) { Fail '--timeout takes seconds' 64 }
            $timeout = $seconds
        }
        default { $positional.Add($arg) }
    }
}

if ($positional.Count -eq 0) { Write-Output $usage; exit 64 }
$command = $positional[0]
$commandArgs = @($positional | Select-Object -Skip 1)

function Argument([int]$index, [string]$name) {
    if ($index -ge $commandArgs.Count) { Fail "$command needs $name`n`n$usage" 64 }
    $commandArgs[$index]
}

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$AE = [System.Windows.Automation.AutomationElement]
$TreeScope = [System.Windows.Automation.TreeScope]

# --- Finding the app ---

function Candidates {
    $wanted = switch ($appName.ToLowerInvariant()) {
        'launcher' { @('lodestone-launcher') }
        'host' { @('lodestone-host') }
        default { @($appName.ToLowerInvariant() -replace '\.exe$', '') }
    }
    @(Get-Process -ErrorAction SilentlyContinue |
        Where-Object { $wanted -contains $_.ProcessName.ToLowerInvariant() -and $_.MainWindowHandle -ne 0 })
}

function TargetPid {
    if ($null -ne $pidArg) {
        if (-not (Get-Process -Id $pidArg -ErrorAction SilentlyContinue)) {
            Fail "no app with pid $pidArg is running" 3
        }
        return $pidArg
    }
    $apps = Candidates
    if ($apps.Count -eq 0) { Fail "$appName isn't running (no window of that name). Start it, or pass --pid." 3 }
    if ($apps.Count -gt 1) {
        $list = ($apps | ForEach-Object { "  $($_.Id)  $($_.Path)" }) -join "`n"
        Fail "several apps match ${appName}; pass --pid:`n$list"
    }
    $apps[0].Id
}

# --- The automation tree ---

# The app's top-level windows: the main one, and any it opened of its own.
function Windows([int]$processId) {
    $condition = [System.Windows.Automation.PropertyCondition]::new($AE::ProcessIdProperty, $processId)
    @($AE::RootElement.FindAll($TreeScope::Children, $condition))
}

function Walk([int]$processId) {
    $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $found = [System.Collections.Generic.List[object]]::new()
    function Visit($element, [int]$depth) {
        $found.Add([pscustomobject]@{ Element = $element; Depth = $depth })
        if ($depth -ge $depthLimit) { return }
        $child = $walker.GetFirstChild($element)
        while ($null -ne $child) {
            Visit $child ($depth + 1)
            $child = $walker.GetNextSibling($child)
        }
    }
    foreach ($window in Windows $processId) { Visit $window 0 }
    $found
}

function Pattern($element, $pattern) {
    $object = $null
    if ($element.TryGetCurrentPattern($pattern, [ref]$object)) { $object } else { $null }
}

$Invoke = [System.Windows.Automation.InvokePattern]::Pattern
$Toggle = [System.Windows.Automation.TogglePattern]::Pattern
$SelectionItem = [System.Windows.Automation.SelectionItemPattern]::Pattern
$ExpandCollapse = [System.Windows.Automation.ExpandCollapsePattern]::Pattern
$ValuePattern = [System.Windows.Automation.ValuePattern]::Pattern
$RangeValue = [System.Windows.Automation.RangeValuePattern]::Pattern

function ValueOf($element) {
    $value = Pattern $element $ValuePattern
    if ($value) { return $value.Current.Value }
    $range = Pattern $element $RangeValue
    if ($range) { return [string]$range.Current.Value }
    $toggle = Pattern $element $Toggle
    if ($toggle) { return [string]$toggle.Current.ToggleState }
    $null
}

function Info($element) {
    $current = $element.Current
    $fields = [ordered]@{ role = $current.ControlType.ProgrammaticName -replace '^ControlType\.', '' }
    if ($current.AutomationId) { $fields.id = $current.AutomationId }
    if ($current.Name) { $fields.name = $current.Name }
    $value = ValueOf $element
    if ($null -ne $value -and $value -ne '') { $fields.value = $value }
    if (-not $current.IsEnabled) { $fields.enabled = $false }
    if ($current.HasKeyboardFocus) { $fields.focused = $true }
    $selection = Pattern $element $SelectionItem
    if ($selection -and $selection.Current.IsSelected) { $fields.selected = $true }
    $expand = Pattern $element $ExpandCollapse
    if ($expand) { $fields.expanded = [string]$expand.Current.ExpandCollapseState }
    $patterns = @($element.GetSupportedPatterns() |
        ForEach-Object { $_.ProgrammaticName -replace 'PatternIdentifiers\.Pattern$', '' } |
        Where-Object { $_ -in 'Invoke', 'Toggle', 'SelectionItem', 'ExpandCollapse', 'Value', 'RangeValue' })
    if ($patterns.Count) { $fields.actions = $patterns }
    $fields
}

function Line($element) {
    $fields = Info $element
    $parts = @($fields.role)
    if ($fields.id) { $parts += "#$($fields.id)" }
    if ($fields.name) { $parts += "`"$($fields.name)`"" }
    if ($fields.Contains('value')) { $parts += "value=`"$($fields.value)`"" }
    if ($fields.Contains('enabled')) { $parts += 'disabled' }
    if ($fields.focused) { $parts += 'focused' }
    if ($fields.selected) { $parts += 'selected' }
    if ($fields.expanded) { $parts += $fields.expanded.ToLowerInvariant() }
    if ($fields.actions) { $parts += "[$($fields.actions -join ' ')]" }
    $parts -join ' '
}

# The control with this automation id. The launcher builds its tree only once an
# automation client first asks, so the first look can come back empty: it asks again
# for a moment before giving up.
function Lookup([int]$processId, [string]$id, [double]$seconds = 2) {
    $condition = [System.Windows.Automation.PropertyCondition]::new($AE::AutomationIdProperty, $id)
    $deadline = [DateTime]::UtcNow.AddSeconds($seconds)
    do {
        foreach ($window in Windows $processId) {
            $hit = $window.FindFirst($TreeScope::Descendants, $condition)
            if ($hit) { return $hit }
        }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    $null
}

function Control([int]$processId, [string]$id) {
    $element = Lookup $processId $id
    if (-not $element) { Fail "no control has the automation id `"$id`" (see ``tree``)" 4 }
    $element
}

# --- Screenshots ---

function Screenshot([int]$processId, [string]$path) {
    $handle = (Get-Process -Id $processId).MainWindowHandle
    if ($handle -eq 0) { Fail 'the app has no window to capture (is it still starting?)' }
    try { Add-Type -AssemblyName System.Drawing.Common } catch { Add-Type -AssemblyName System.Drawing }
    if (-not ('UiDriveCapture' -as [type])) {
        Add-Type @'
using System; using System.Runtime.InteropServices;
public static class UiDriveCapture {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out RECT rect);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr window, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr window);
}
'@
    }
    if ([UiDriveCapture]::IsIconic($handle)) { Fail 'the window is minimized; restore it to capture it' }
    $rect = [UiDriveCapture+RECT]::new()
    [void][UiDriveCapture]::GetWindowRect($handle, [ref]$rect)
    $bitmap = [System.Drawing.Bitmap]::new($rect.Right - $rect.Left, $rect.Bottom - $rect.Top)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $dc = $graphics.GetHdc()
    # PW_RENDERFULLCONTENT (2): asks the window to draw itself, so a GPU-drawn window
    # comes out whole even when other windows cover it.
    $ok = [UiDriveCapture]::PrintWindow($handle, $dc, 2)
    $graphics.ReleaseHdc($dc)
    $graphics.Dispose()
    if (-not $ok) { $bitmap.Dispose(); Fail 'PrintWindow failed' }
    $full = [System.IO.Path]::GetFullPath($path)
    $bitmap.Save($full, [System.Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()
    Write-Output $full
}

function Check([scriptblock]$action, [string]$what) {
    try { & $action } catch { Fail "$what failed: $($_.Exception.Message)" }
}

# --- Commands ---

switch ($command) {
    'apps' {
        $apps = @(Candidates | ForEach-Object {
                [ordered]@{ pid = $_.Id; name = $_.MainWindowTitle; path = $_.Path }
            })
        if ($json) { ConvertTo-Json -InputObject $apps -Depth 3 }
        elseif ($apps.Count -eq 0) { Fail "no app matches $appName" 3 }
        else { $apps | ForEach-Object { "$($_.pid)  $($_.name)  $($_.path)" } }
    }
    'tree' {
        $processId = TargetPid
        $nodes = Walk $processId
        # The first look asks the launcher to build its tree; look again once it has.
        if (-not ($nodes | Where-Object { $_.Element.Current.AutomationId })) {
            Start-Sleep -Milliseconds 500
            $nodes = Walk $processId
        }
        if ($json) {
            $list = @($nodes | ForEach-Object { $fields = Info $_.Element; $fields.depth = $_.Depth; $fields })
            ConvertTo-Json -InputObject $list -Depth 4
        } else {
            $nodes | ForEach-Object { ('  ' * $_.Depth) + (Line $_.Element) }
        }
    }
    'find' {
        $element = Control (TargetPid) (Argument 0 'an automation id')
        if ($json) { ConvertTo-Json -InputObject (Info $element) -Depth 3 } else { Line $element }
    }
    'press' {
        $id = Argument 0 'an automation id'
        $element = Control (TargetPid) $id
        $invoke = Pattern $element $Invoke
        $toggle = Pattern $element $Toggle
        $select = Pattern $element $SelectionItem
        $expand = Pattern $element $ExpandCollapse
        Check {
            if ($invoke) { $invoke.Invoke() }
            elseif ($toggle) { $toggle.Toggle() }
            elseif ($select) { $select.Select() }
            elseif ($expand) {
                if ($expand.Current.ExpandCollapseState -eq 'Expanded') { $expand.Collapse() } else { $expand.Expand() }
            } else { throw 'it has no Invoke, Toggle, SelectionItem or ExpandCollapse pattern' }
        } "press $id"
    }
    'select' {
        $id = Argument 0 'an automation id'
        $element = Control (TargetPid) $id
        $select = Pattern $element $SelectionItem
        $invoke = Pattern $element $Invoke
        Check {
            if ($select) { $select.Select() }
            elseif ($invoke) { $invoke.Invoke() }
            else { throw 'it has no SelectionItem or Invoke pattern' }
        } "select $id"
    }
    'set' {
        $id = Argument 0 'an automation id'
        $text = Argument 1 'a value'
        $element = Control (TargetPid) $id
        $value = Pattern $element $ValuePattern
        $range = Pattern $element $RangeValue
        $number = 0.0
        Check {
            if ($range -and [double]::TryParse($text, [ref]$number)) { $range.SetValue($number) }
            elseif ($value) { $value.SetValue($text) }
            else { throw 'it has no Value or RangeValue pattern' }
        } "set $id"
    }
    'value' {
        $element = Control (TargetPid) (Argument 0 'an automation id')
        $value = ValueOf $element
        if ($null -eq $value) { '' } else { $value }
    }
    'focus' {
        $id = Argument 0 'an automation id'
        $element = Control (TargetPid) $id
        Check { $element.SetFocus() } "focus $id"
    }
    { $_ -in 'increment', 'decrement' } {
        $id = Argument 0 'an automation id'
        $element = Control (TargetPid) $id
        $range = Pattern $element $RangeValue
        if (-not $range) { Fail "$command $id failed: it has no RangeValue pattern" }
        $step = if ($range.Current.SmallChange -gt 0) { $range.Current.SmallChange } else { 1 }
        $target = if ($command -eq 'increment') { $range.Current.Value + $step } else { $range.Current.Value - $step }
        $target = [Math]::Min($range.Current.Maximum, [Math]::Max($range.Current.Minimum, $target))
        Check { $range.SetValue($target) } "$command $id"
    }
    'wait' {
        $id = Argument 0 'an automation id'
        $element = Lookup (TargetPid) $id $timeout
        if (-not $element) { Fail "`"$id`" didn't appear within $timeout seconds" 4 }
        Line $element
    }
    'gone' {
        $id = Argument 0 'an automation id'
        $processId = TargetPid
        $deadline = [DateTime]::UtcNow.AddSeconds($timeout)
        while (Lookup $processId $id 0) {
            if ([DateTime]::UtcNow -ge $deadline) { Fail "`"$id`" is still there after $timeout seconds" }
            Start-Sleep -Milliseconds 200
        }
    }
    'screenshot' { Screenshot (TargetPid) (Argument 0 'a file name') }
    default { Fail "unknown command $command`n`n$usage" 64 }
}
