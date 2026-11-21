# Runs the Windows release checklist for one build of Lodestone Launcher or Lodestone
# Host, inside the test VM, in its signed-in desktop session. windows_vm.py copies it
# in with the build and ui_drive.ps1, runs it (`windows_vm.py test`), and copies the
# results out: results.json, the screenshots and the logs, in -Out.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File windows_vm_test.ps1 `
#       -Product launcher -Dir C:\lodestone-test -Build 4487 -Commit abc1234
#
# What it checks, as the release checklist says (publish_release.py's TEST_CHECKLIST
# and HOST_TEST_CHECKLIST):
#   standalone  the -standalone exe opens its window with its own empty data folders,
#               and Settings shows "Build <build>" (the Host's runs the dry run's core
#               and signs in); it closes cleanly (the Host's core stops with it).
#   install     the installer installs for this user without an administrator prompt,
#               Settings > Apps lists it with "(build <build>)", the Start Menu
#               shortcut opens it.
#   uninstall   a silent uninstall removes the program and keeps the data folder.
# It never presses anything that starts a game or a Minecraft server, and never signs
# in to anything.

param(
    [Parameter(Mandatory)][ValidateSet('launcher', 'host')][string]$Product,
    [Parameter(Mandatory)][string]$Dir,
    [Parameter(Mandatory)][string]$Build,
    [string]$Commit = '',
    [string]$Out = '',
    [switch]$SkipInstaller
)

$ErrorActionPreference = 'Stop'
# vmrun and cmd can hand over the last argument with whitespace after it.
$Build = $Build.Trim(); $Commit = $Commit.Trim()
if (-not $Out) { $Out = Join-Path $Dir 'results' }
if (Test-Path $Out) { Remove-Item -Recurse -Force $Out }
New-Item -ItemType Directory -Force $Out | Out-Null
Start-Transcript (Join-Path $Out 'transcript.txt') | Out-Null

$UiDrive = Join-Path $Dir 'ui_drive.ps1'
$AppName = @{ launcher = 'Lodestone Launcher'; host = 'Lodestone Host' }[$Product]
$ExeName = @{ launcher = 'lodestone-launcher.exe'; host = 'lodestone-host.exe' }[$Product]
$SettingsPage = @{ launcher = 'settings-nav-general'; host = 'settings-nav-updates' }[$Product]
$Standalone = Get-ChildItem $Dir -Filter "lodestone-$Product-*-standalone.exe" | Select-Object -First 1
$Setup = Get-ChildItem $Dir -Filter "lodestone-$Product-*-setup.exe" | Select-Object -First 1
$Core = Get-ChildItem $Dir -Filter "lodestone-host-*-core.exe" | Select-Object -First 1

# Test data folders go inside the account's own folder: on Windows the core only lets
# the Host claim it (sign in without a password) when its data folder is there.
$DataRoot = Join-Path $env:LOCALAPPDATA 'lodestone-test'
if (Test-Path $DataRoot) { Remove-Item -Recurse -Force $DataRoot }
New-Item -ItemType Directory -Force $DataRoot | Out-Null

$results = [System.Collections.Generic.List[object]]::new()
function Record([string]$Step, [string]$Status, [string]$Detail, [string[]]$Files = @()) {
    $results.Add([ordered]@{ step = $Step; status = $Status; detail = $Detail; files = $Files })
    Write-Host "[$Status] $Step - $Detail"
}

# ui_drive.ps1 in this process's session; returns its exit code and output.
# A failing ui_drive writes to stderr; under 'Stop' that would throw here instead of
# returning its exit code, so this function alone carries on.
function UiDrive([string[]]$UiArgs) {
    $ErrorActionPreference = 'Continue'
    $output = & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $UiDrive @UiArgs 2>&1
    $code = $LASTEXITCODE
    return [pscustomobject]@{ Code = $code; Text = (($output | ForEach-Object { "$_" }) -join "`n").Trim() }
}

# "Build 4487" in the Launcher's Settings, "Build: 4487" in the Host's.
$BuildPattern = "\bBuild:?\s*$([regex]::Escape($Build))\b"

function ExitCodeOf([System.Diagnostics.Process]$Process) {
    try { return $Process.ExitCode } catch { return '?' }
}

Add-Type -AssemblyName System.Drawing, System.Windows.Forms
function ScreenShot([string]$Name) {
    $path = Join-Path $Out "$Name.png"
    try {
        $bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
        $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
        $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
        $graphics.Dispose(); $bitmap.Dispose()
        return "$Name.png"
    } catch {
        Write-Host "screenshot $Name failed: $_"
        return $null
    }
}

# Waits for the app's window (its Settings control), then opens Settings on the page
# with the build, and looks for "Build <build>" in the controls' names and values.
function CheckApp([string]$Step, [int]$AppPid) {
    $files = @()
    $wait = UiDrive @('--app', $Product, '--pid', "$AppPid", 'wait', 'settings', '--timeout', '90')
    if ($wait.Code -ne 0) {
        $files += ScreenShot "$Step-no-window"
        Record "$Step-window" 'fail' "no window with a Settings control within 90 s: $($wait.Text)" $files
        return $false
    }
    Start-Sleep -Seconds 3
    $files += ScreenShot "$Step-window"
    Record "$Step-window" 'pass' 'the window opened' $files

    $press = UiDrive @('--app', $Product, '--pid', "$AppPid", 'press', 'settings')
    $page = UiDrive @('--app', $Product, '--pid', "$AppPid", 'wait', $SettingsPage, '--timeout', '15')
    if ($page.Code -eq 0) {
        UiDrive @('--app', $Product, '--pid', "$AppPid", 'select', $SettingsPage) | Out-Null
    }
    Start-Sleep -Seconds 3
    $tree = UiDrive @('--app', $Product, '--pid', "$AppPid", 'tree')
    Set-Content (Join-Path $Out "$Step-settings-tree.txt") $tree.Text
    $shot = ScreenShot "$Step-settings"
    $window = UiDrive @('--app', $Product, '--pid', "$AppPid", 'screenshot', (Join-Path $Out "$Step-settings-window.png"))
    $shots = @($shot) + $(if ($window.Code -eq 0) { "$Step-settings-window.png" } else { @() })
    if ($press.Code -ne 0) {
        Record "$Step-build" 'fail' "couldn't open Settings: $($press.Text)" $shots
    } elseif ($tree.Text -match $BuildPattern) {
        $line = ($tree.Text -split "`n" | Where-Object { $_ -match $BuildPattern } | Select-Object -First 1).Trim()
        $commitNote = if (-not $Commit) { '' }
            elseif ($tree.Text -match [regex]::Escape($Commit)) { "; commit $Commit shown too" }
            else { " (commit $Commit isn't in Settings' controls: check the screenshot)" }
        Record "$Step-build" 'pass' "Settings shows: $line$commitNote" $shots
    } else {
        Record "$Step-build" 'check' "build $Build isn't in Settings' controls; read it off the screenshot" $shots
    }
    return $true
}

# Closes the app through its window, as a person would. The Launcher quits; the Host
# stays in the tray (closing its window minimizes it), and its Quit is only in the
# tray's menu, which UI Automation can't reach, so the Host is then ended and its core
# has to stop with it (it watches the Host through its standard input).
function CloseApp([string]$Step, [System.Diagnostics.Process]$Process, [string]$CorePath) {
    $Process.Refresh()
    if ($Process.HasExited) {
        Record "$Step-close" 'fail' "it had already exited (code $(ExitCodeOf $Process))"
        return
    }
    if (-not $Process.CloseMainWindow()) {
        Record "$Step-close" 'fail' 'found no main window to close; stopping it'
        Stop-Process -Id $Process.Id -Force
        $null = $Process.WaitForExit(10000)
        WaitCore $Step $CorePath
        return
    }
    if ($Product -eq 'host') {
        Start-Sleep -Seconds 5
        $Process.Refresh()
        if ($Process.HasExited) {
            Record "$Step-close" 'fail' "closing the window quit the Host (code $(ExitCodeOf $Process)); it should stay in the tray"
        } else {
            Record "$Step-close" 'pass' 'closing the window kept the Host running in the tray'
            Record "$Step-quit" 'check' "Quit from the tray icon's menu isn't reachable from here: check it by hand"
            Stop-Process -Id $Process.Id -Force
            $null = $Process.WaitForExit(10000)
        }
    } elseif (-not $Process.WaitForExit(30000)) {
        Record "$Step-close" 'fail' "still running 30 s after its window was closed; stopping it"
        Stop-Process -Id $Process.Id -Force
    } else {
        Record "$Step-close" 'pass' "closed through its window (exit code $(ExitCodeOf $Process))"
    }
    WaitCore $Step $CorePath
}

# The Host's core watches the Host through its standard input and stops when it goes.
function WaitCore([string]$Step, [string]$CorePath) {
    if (-not $CorePath) { return }
    $deadline = (Get-Date).AddSeconds(30)
    do {
        $cores = @(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -eq $CorePath })
        if (-not $cores) { break }
        Start-Sleep -Seconds 1
    } while ((Get-Date) -lt $deadline)
    if ($cores) {
        Record "$Step-core-stops" 'fail' "the core ($CorePath) still runs 30 s after the Host ended"
        $cores | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
    } else {
        Record "$Step-core-stops" 'pass' 'the core stopped when the Host ended'
    }
}

# --- standalone: the dry run's -standalone exe, with empty data folders ---
try {
    if (-not $Standalone) { throw "no lodestone-$Product-*-standalone.exe in $Dir" }
    $data = Join-Path $DataRoot 'standalone'
    New-Item -ItemType Directory -Force $data | Out-Null
    $info = New-Object System.Diagnostics.ProcessStartInfo $Standalone.FullName
    $info.UseShellExecute = $false
    $info.WorkingDirectory = $Dir
    $corePath = $null
    if ($Product -eq 'launcher') {
        $info.EnvironmentVariables['LODESTONE_LAUNCHER_DATA_DIR'] = Join-Path $data 'launcher'
    } else {
        if (-not $Core) { throw "no lodestone-host-*-core.exe in $Dir" }
        $corePath = $Core.FullName
        $info.EnvironmentVariables['LODESTONE_PATH'] = Join-Path $data 'core'
        $info.EnvironmentVariables['LODESTONE_HOST_DATA_DIR'] = Join-Path $data 'host'
        $info.EnvironmentVariables['LODESTONE_HOST_CORE'] = $corePath
    }
    $process = [System.Diagnostics.Process]::Start($info)
    Record 'standalone-start' 'pass' "started $($Standalone.Name) (pid $($process.Id))"
    if (CheckApp 'standalone' $process.Id) {
        CloseApp 'standalone' $process $corePath
    } else {
        $process.Refresh()
        if ($process.HasExited) {
            Record 'standalone-exit' 'fail' "it exited with code $(ExitCodeOf $process)"
        } else {
            Stop-Process -Id $process.Id -Force
        }
        WaitCore 'standalone' $corePath
    }
} catch {
    Record 'standalone' 'fail' "$_"
}

# --- install, the Start Menu shortcut, Settings > Apps, uninstall ---
if ($SkipInstaller) {
    Record 'install' 'skip' 'skipped (-SkipInstaller)'
} elseif (-not $Setup) {
    Record 'install' 'skip' "no lodestone-$Product-*-setup.exe in $Dir"
} else {
    $script:installFiles = @()
    try {
        $log = Join-Path $Out 'setup.log'
        # /CURRENTUSER: no administrator rights. A prompt for them would sit
        # unanswered, so the timeout catches it.
        $setup = Start-Process $Setup.FullName -PassThru -ArgumentList @(
            '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/CURRENTUSER', "/LOG=`"$log`"")
        $null = $setup.Handle  # without it, ExitCode can come back empty
        if (-not $setup.WaitForExit(300000)) {
            $script:installFiles = @(ScreenShot 'install-stuck') | Where-Object { $_ }
            # The whole tree: Setup's stub runs Inno's setup.tmp, which does the work.
            & taskkill.exe /PID $setup.Id /T /F | Out-Null
            throw "Setup didn't finish in 5 minutes (an administrator prompt?); see install-stuck.png"
        }
        if ($setup.ExitCode -ne 0) { throw "Setup exited with code $($setup.ExitCode); see setup.log" }

        $key = Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' |
            Get-ItemProperty | Where-Object { $_.DisplayName -eq $AppName } | Select-Object -First 1
        if (-not $key) { throw "Settings > Apps has no `"$AppName`" (no uninstall entry for this user)" }
        $machine = Get-ChildItem 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue |
            Get-ItemProperty | Where-Object { $_.DisplayName -eq $AppName }
        $where = $key.InstallLocation
        $versionOk = $key.DisplayVersion -match "\(build $Build\)"
        Record 'install' $(if ($versionOk -and -not $machine) { 'pass' } else { 'fail' }) (
            "installed for this user in $where; Settings > Apps: `"$AppName`" $($key.DisplayVersion)" +
            $(if (-not $versionOk) { " (expected build $Build)" } else { '' }) +
            $(if ($machine) { '; it also made a machine-wide entry' } else { '' }))

        $shortcut = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\$AppName.lnk"
        if (-not (Test-Path $shortcut)) { throw "no Start Menu shortcut at $shortcut" }
        $installedExe = Join-Path $where $ExeName
        if ($Product -eq 'host') {
            # The installed Host's core gets a data folder of its own (the shortcut's
            # program inherits this), so no server left in the VM's ~\.lodestone can
            # start. The Host's own data stays where installs put it, for the
            # uninstall check.
            $coreData = Join-Path $DataRoot 'installed-core'
            New-Item -ItemType Directory -Force $coreData | Out-Null
            $env:LODESTONE_PATH = $coreData
        }
        Start-Process $shortcut
        $deadline = (Get-Date).AddSeconds(30)
        do {
            Start-Sleep -Seconds 1
            $installed = Get-Process | Where-Object { $_.Path -eq $installedExe } | Select-Object -First 1
        } while (-not $installed -and (Get-Date) -lt $deadline)
        if (-not $installed) { throw "the Start Menu shortcut didn't start $installedExe" }
        $null = $installed.Handle  # so ExitCode can be read once it exits
        Record 'shortcut' 'pass' "the Start Menu shortcut started $installedExe (pid $($installed.Id))"
        $installedCore = if ($Product -eq 'host') { Join-Path $where 'lodestone_core.exe' } else { $null }
        if (CheckApp 'installed' $installed.Id) {
            CloseApp 'installed' $installed $installedCore
        } else {
            Stop-Process -Id $installed.Id -Force -ErrorAction SilentlyContinue
            WaitCore 'installed' $installedCore
        }

        # Uninstall silently: the program goes, the data stays (shared.iss).
        $dataDir = Join-Path $env:APPDATA $AppName
        $uninstaller = ($key.UninstallString -replace '^"([^"]+)".*$', '$1')
        $un = Start-Process $uninstaller -PassThru -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART')
        $null = $un.Handle
        if (-not $un.WaitForExit(300000)) { Stop-Process -Id $un.Id -Force; throw "the uninstaller didn't finish in 5 minutes" }
        # The uninstaller copies itself to a temp folder and exits; wait for the copy.
        $deadline = (Get-Date).AddSeconds(120)
        while ((Test-Path $where) -and (Get-Date) -lt $deadline) { Start-Sleep -Seconds 2 }
        $left = @()
        if (Test-Path $installedExe) { $left += "the program is still at $installedExe" }
        elseif (Test-Path $where) {
            $rest = (Get-ChildItem $where -Recurse -File -ErrorAction SilentlyContinue | Select-Object -First 5 -ExpandProperty Name) -join ', '
            $left += "the install folder $where is still there ($rest)"
        }
        if (Test-Path $shortcut) { $left += 'the Start Menu shortcut is still there' }
        $stillListed = Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' |
            Get-ItemProperty | Where-Object { $_.DisplayName -eq $AppName }
        if ($stillListed) { $left += 'Settings > Apps still lists it' }
        $kept = Test-Path $dataDir
        if ($left) {
            Record 'uninstall' 'fail' ($left -join '; ')
        } elseif (-not $kept) {
            Record 'uninstall' 'fail' "removed, but the data folder $dataDir went too (a silent uninstall keeps it)"
        } else {
            Record 'uninstall' 'pass' "removed the program, shortcut and Apps entry; kept $dataDir"
        }
    } catch {
        Record 'install' 'fail' "$_" $script:installFiles
    }
}

# The apps' own logs (the Host's in its data folder's logs\, the core's in log\).
$logs = Join-Path $Out 'logs'
foreach ($folder in @($DataRoot, (Join-Path $env:APPDATA $AppName))) {
    if (Test-Path $folder) {
        Get-ChildItem $folder -Recurse -File -Include *.log, *.log.*, session.json -ErrorAction SilentlyContinue |
            ForEach-Object {
                $name = ($_.FullName.Substring($folder.Length).TrimStart('\') -replace '[\\/:]', '_')
                $prefix = if ($folder -eq $DataRoot) { 'test' } else { 'installed' }
                New-Item -ItemType Directory -Force $logs | Out-Null
                Copy-Item $_.FullName (Join-Path $logs "$prefix-$name") -ErrorAction SilentlyContinue
            }
    }
}

$summary = [ordered]@{
    product = $Product; build = $Build; commit = $Commit
    windows = (Get-CimInstance Win32_OperatingSystem).Caption + ' ' + [Environment]::OSVersion.Version
    arch = $env:PROCESSOR_ARCHITECTURE
    results = $results
}
$summary | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $Out 'results.json') -Encoding UTF8
Stop-Transcript | Out-Null
$failed = @($results | Where-Object { $_.status -eq 'fail' }).Count
exit $(if ($failed) { 1 } else { 0 })
