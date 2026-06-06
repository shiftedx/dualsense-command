<#
.SYNOPSIS
Preflights or runs the DSCC Windows MSI smoke path.

.DESCRIPTION
By default this script checks the MSI paths and prints the install/upgrade/
uninstall checklist without mutating the machine. Pass -Execute on a clean
Windows test account or VM to run msiexec and verify the per-user install
payload, same-version or baseline-to-current upgrade, uninstall cleanup, and
basic orphan-process conditions.
#>

[CmdletBinding()]
param(
    [string]$MsiPath,
    [string]$BaselineMsiPath,
    [switch]$Execute,
    [switch]$SkipUpgrade,
    [switch]$SkipLaunchCheck,
    [switch]$KeepInstalled,
    [switch]$AllowExistingInstall,
    [switch]$AllowExistingProcesses,
    [int]$TimeoutSeconds = 120,
    [string]$LogDirectory,
    [ValidateSet("0", "1")]
    [string]$StartWithWindows = "1",
    [ValidateSet("0", "1")]
    [string]$CreateDesktopShortcut = "0",
    [ValidateSet("0", "1")]
    [string]$LaunchAfterInstall = "1"
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = "Stop"

function Resolve-RepoRoot {
    $scriptDir = Split-Path -Parent $PSCommandPath
    return (Resolve-Path (Join-Path $scriptDir "..")).Path
}

function Resolve-DefaultMsi {
    param([string]$RepoRoot)

    $candidateRoots = @(
        (Join-Path $RepoRoot "target\installer"),
        (Join-Path $RepoRoot "target\release-artifacts\windows")
    )

    $candidates = @()
    foreach ($root in $candidateRoots) {
        if (Test-Path -LiteralPath $root) {
            $candidates += Get-ChildItem -LiteralPath $root -Filter "*.msi" -File -ErrorAction SilentlyContinue
        }
    }

    if ($candidates.Count -eq 0) {
        throw "No MSI was supplied and no MSI was found under target\installer or target\release-artifacts\windows. Build one with packaging\package-msi.ps1 or pass -MsiPath."
    }

    return ($candidates | Sort-Object LastWriteTimeUtc, Name -Descending | Select-Object -First 1).FullName
}

function Resolve-MsiFile {
    param(
        [string]$Path,
        [string]$Label
    )

    if ([string]::IsNullOrWhiteSpace($Path)) {
        throw "$Label path is empty."
    }

    $resolved = Resolve-Path -LiteralPath $Path -ErrorAction Stop
    $item = Get-Item -LiteralPath $resolved.Path -ErrorAction Stop
    if ($item.PSIsContainer) {
        throw "$Label is a directory, not an MSI: $($item.FullName)"
    }
    if ($item.Extension -ne ".msi") {
        throw "$Label must be an .msi file: $($item.FullName)"
    }
    if ($item.Length -le 0) {
        throw "$Label is empty: $($item.FullName)"
    }

    return [pscustomobject]@{
        Label = $Label
        Path = $item.FullName
        Name = $item.Name
        Bytes = $item.Length
        Sha256 = (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}

function Write-Step {
    param([string]$Message)

    Write-Host "[dscc-msi-smoke] $Message"
}

function Write-MsiSummary {
    param([psobject]$Msi)

    Write-Step ("{0}: {1}" -f $Msi.Label, $Msi.Path)
    Write-Host ("  Size: {0} bytes" -f $Msi.Bytes)
    Write-Host ("  SHA256: {0}" -f $Msi.Sha256)
}

function Assert-WindowsHost {
    if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
        throw "The executable MSI smoke test must run on Windows."
    }
}

function Get-DsccInstallFolder {
    if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
        throw "LOCALAPPDATA is not set."
    }
    return (Join-Path $env:LOCALAPPDATA "Programs\DualSense Command Center")
}

function Get-DsccStartMenuFolder {
    if ([string]::IsNullOrWhiteSpace($env:APPDATA)) {
        throw "APPDATA is not set."
    }
    return (Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\DualSense Command Center")
}

function Get-DsccDesktopShortcut {
    $desktop = [Environment]::GetFolderPath("DesktopDirectory")
    if ([string]::IsNullOrWhiteSpace($desktop)) {
        throw "DesktopDirectory could not be resolved."
    }
    return (Join-Path $desktop "DualSense Command Center.lnk")
}

function Get-RunAtLoginValue {
    $keyPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
    if (-not (Test-Path -Path $keyPath)) {
        return $null
    }

    $property = Get-ItemProperty -Path $keyPath -Name "DualSense Command Center" -ErrorAction SilentlyContinue
    if ($null -eq $property) {
        return $null
    }

    return $property.'DualSense Command Center'
}

function Get-DsccProcessSnapshot {
    $processes = Get-Process -Name "dscc-agent", "dscc-tray" -ErrorAction SilentlyContinue
    return @(
        $processes |
            ForEach-Object {
                $path = "<unavailable>"
                try {
                    if (-not [string]::IsNullOrWhiteSpace($_.Path)) {
                        $path = $_.Path
                    }
                } catch {
                    $path = "<unavailable>"
                }

                [pscustomobject]@{
                    ProcessName = $_.ProcessName
                    Id = $_.Id
                    Path = $path
                }
            } |
            Sort-Object ProcessName, Id
    )
}

function Format-ProcessSnapshot {
    param([object[]]$Processes)

    if ($Processes.Count -eq 0) {
        return "  (none)"
    }

    return (($Processes | ForEach-Object {
        "  {0} pid={1} path={2}" -f $_.ProcessName, $_.Id, $_.Path
    }) -join [Environment]::NewLine)
}

function Assert-CleanStart {
    param(
        [switch]$AllowExistingInstall,
        [switch]$AllowExistingProcesses
    )

    $processes = @(Get-DsccProcessSnapshot)
    if ($processes.Count -gt 0) {
        $details = Format-ProcessSnapshot -Processes $processes
        if (-not $AllowExistingProcesses) {
            throw "DSCC processes are already running. Stop them first or pass -AllowExistingProcesses for a dirty-box smoke.`n$details"
        }
        Write-Warning "Continuing with existing DSCC processes because -AllowExistingProcesses was supplied."
        Write-Warning $details
    }

    $markers = @()
    $installFolder = Get-DsccInstallFolder
    $startMenuFolder = Get-DsccStartMenuFolder
    $desktopShortcut = Get-DsccDesktopShortcut
    if (Test-Path -LiteralPath $installFolder) {
        $markers += $installFolder
    }
    if (Test-Path -LiteralPath $startMenuFolder) {
        $markers += $startMenuFolder
    }
    if (Test-Path -LiteralPath $desktopShortcut) {
        $markers += $desktopShortcut
    }
    if ($null -ne (Get-RunAtLoginValue)) {
        $markers += "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run\DualSense Command Center"
    }

    if ($markers.Count -gt 0) {
        if (-not $AllowExistingInstall) {
            throw "Existing DSCC install markers were found. Use a clean test account/VM, uninstall first, or pass -AllowExistingInstall.`n  $($markers -join "`n  ")"
        }
        Write-Warning "Continuing with existing install markers because -AllowExistingInstall was supplied."
        Write-Warning ("  {0}" -f ($markers -join "`n  "))
    }
}

function Invoke-MsiAction {
    param(
        [ValidateSet("Install", "Uninstall")]
        [string]$Action,
        [string]$Path,
        [string]$LogPath,
        [hashtable]$Properties = @{}
    )

    $msiexec = Join-Path $env:SystemRoot "System32\msiexec.exe"
    if (-not (Test-Path -LiteralPath $msiexec)) {
        $msiexec = "msiexec.exe"
    }

    if ($Action -eq "Install") {
        $arguments = @("/i", $Path, "/qn", "/norestart", "/l*v", $LogPath)
        foreach ($entry in $Properties.GetEnumerator() | Sort-Object Name) {
            $arguments += ("{0}={1}" -f $entry.Name, $entry.Value)
        }
    } else {
        $arguments = @("/x", $Path, "/qn", "/norestart", "/l*v", $LogPath)
    }

    $argumentLine = ($arguments | ForEach-Object {
        $argument = [string]$_
        if ($argument -match '[\s"]') {
            '"' + ($argument -replace '"', '\"') + '"'
        } else {
            $argument
        }
    }) -join " "

    Write-Step ("{0} {1}" -f $Action, $Path)
    Write-Host ("  Log: {0}" -f $LogPath)
    $process = Start-Process -FilePath $msiexec -ArgumentList $argumentLine -Wait -PassThru -WindowStyle Hidden
    $exitCode = $process.ExitCode

    if ($exitCode -eq 0) {
        return
    }
    if ($exitCode -eq 3010) {
        Write-Warning "$Action returned 3010, reboot required. Continuing smoke checks."
        return
    }

    throw "$Action failed with msiexec exit code $exitCode. See $LogPath"
}

function Assert-InstalledPayload {
    param(
        [ValidateSet("0", "1")]
        [string]$ExpectedStartWithWindows,
        [ValidateSet("0", "1")]
        [string]$ExpectedDesktopShortcut
    )

    $installFolder = Get-DsccInstallFolder
    $startMenuFolder = Get-DsccStartMenuFolder
    $desktopShortcut = Get-DsccDesktopShortcut
    $expectedFiles = @(
        (Join-Path $installFolder "dscc-agent.exe"),
        (Join-Path $installFolder "dscc-tray.exe"),
        (Join-Path $installFolder "dscc-cli.exe"),
        (Join-Path $installFolder "Stop DSCC.cmd"),
        (Join-Path $installFolder "Backup DSCC State.cmd"),
        (Join-Path $installFolder "README_TESTING.txt"),
        (Join-Path $installFolder "LICENSE.txt"),
        (Join-Path $installFolder "web\dist\index.html")
    )

    $missing = @($expectedFiles | Where-Object { -not (Test-Path -LiteralPath $_) })
    if ($missing.Count -gt 0) {
        throw "Installed payload is missing expected files:`n  $($missing -join "`n  ")"
    }

    if (-not (Test-Path -LiteralPath $startMenuFolder)) {
        throw "Start menu folder was not created: $startMenuFolder"
    }

    $shortcuts = @(Get-ChildItem -LiteralPath $startMenuFolder -Filter "*.lnk" -File -ErrorAction SilentlyContinue)
    if ($shortcuts.Count -eq 0) {
        throw "Start menu folder contains no shortcuts: $startMenuFolder"
    }

    $runValue = Get-RunAtLoginValue
    if ($ExpectedStartWithWindows -eq "1" -and ([string]::IsNullOrWhiteSpace($runValue) -or ($runValue -notmatch "dscc-tray\.exe"))) {
        throw "Run-at-login value is missing or does not point at dscc-tray.exe."
    }
    if ($ExpectedStartWithWindows -eq "0" -and $null -ne $runValue) {
        throw "Run-at-login value exists even though StartWithWindows was disabled: $runValue"
    }

    if ($ExpectedDesktopShortcut -eq "1" -and -not (Test-Path -LiteralPath $desktopShortcut)) {
        throw "Desktop shortcut was not created: $desktopShortcut"
    }
    if ($ExpectedDesktopShortcut -eq "0" -and (Test-Path -LiteralPath $desktopShortcut)) {
        throw "Desktop shortcut exists even though CreateDesktopShortcut was disabled: $desktopShortcut"
    }

    Write-Step "Installed payload checks passed."
}

function Assert-UninstalledPayload {
    $installFolder = Get-DsccInstallFolder
    $startMenuFolder = Get-DsccStartMenuFolder
    $desktopShortcut = Get-DsccDesktopShortcut
    $leftovers = @()

    foreach ($path in @(
        (Join-Path $installFolder "dscc-agent.exe"),
        (Join-Path $installFolder "dscc-tray.exe"),
        (Join-Path $installFolder "dscc-cli.exe"),
        (Join-Path $installFolder "LICENSE.txt"),
        (Join-Path $installFolder "web\dist\index.html")
    )) {
        if (Test-Path -LiteralPath $path) {
            $leftovers += $path
        }
    }

    if (Test-Path -LiteralPath $startMenuFolder) {
        $shortcuts = @(Get-ChildItem -LiteralPath $startMenuFolder -Filter "*.lnk" -File -ErrorAction SilentlyContinue)
        foreach ($shortcut in $shortcuts) {
            $leftovers += $shortcut.FullName
        }
    }

    if ($null -ne (Get-RunAtLoginValue)) {
        $leftovers += "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run\DualSense Command Center"
    }
    if (Test-Path -LiteralPath $desktopShortcut) {
        $leftovers += $desktopShortcut
    }

    if ($leftovers.Count -gt 0) {
        throw "Uninstall left installer-owned payload behind:`n  $($leftovers -join "`n  ")"
    }

    Write-Step "Uninstall filesystem and registry checks passed."
}

function Wait-ForDsccProcesses {
    param(
        [string[]]$Names,
        [int]$TimeoutSeconds
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    do {
        $processes = @(Get-DsccProcessSnapshot)
        $foundAll = $true
        foreach ($name in $Names) {
            $matches = @($processes | Where-Object { $_.ProcessName -eq $name })
            if ($matches.Count -eq 0) {
                $foundAll = $false
            }
        }

        if ($foundAll) {
            return $processes
        }

        Start-Sleep -Seconds 2
    } while ((Get-Date) -lt $deadline)

    $details = Format-ProcessSnapshot -Processes @(Get-DsccProcessSnapshot)
    throw "Timed out waiting for DSCC processes: $($Names -join ', '). Current processes:`n$details"
}

function Assert-DsccProcessesRunFromInstallFolder {
    $processes = @(Get-DsccProcessSnapshot)
    $installRoot = [System.IO.Path]::GetFullPath((Get-DsccInstallFolder).TrimEnd("\") + "\")

    foreach ($group in ($processes | Group-Object ProcessName)) {
        if ($group.Count -gt 1) {
            throw "Expected at most one $($group.Name) process, found $($group.Count).`n$(Format-ProcessSnapshot -Processes $processes)"
        }
    }

    foreach ($process in $processes) {
        if ([string]::IsNullOrWhiteSpace($process.Path) -or $process.Path -eq "<unavailable>") {
            Write-Warning "Could not read process path for $($process.ProcessName) pid=$($process.Id)."
            continue
        }

        $processPath = [System.IO.Path]::GetFullPath($process.Path)
        if (-not $processPath.StartsWith($installRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "DSCC process is not running from the current install folder: $($process.ProcessName) pid=$($process.Id) path=$processPath install=$installRoot"
        }
    }

    Write-Step "Process ownership checks passed."
}

function Wait-ForNoDsccProcesses {
    param([int]$TimeoutSeconds)

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    do {
        $processes = @(Get-DsccProcessSnapshot)
        if ($processes.Count -eq 0) {
            Write-Step "No DSCC processes remain after uninstall."
            return
        }
        Start-Sleep -Seconds 2
    } while ((Get-Date) -lt $deadline)

    $details = Format-ProcessSnapshot -Processes @(Get-DsccProcessSnapshot)
    throw "DSCC processes are still running after uninstall:`n$details"
}

function Write-SmokeChecklist {
    param(
        [psobject]$BaselineMsi,
        [psobject]$CurrentMsi,
        [string]$LogDirectory
    )

    Write-Step "Preflight complete. Add -Execute on a clean Windows test account or VM to run msiexec."
    Write-Host ""
    Write-Host "Smoke checklist:"
    Write-Host "  1. Confirm no existing DSCC install markers or dscc-agent/dscc-tray processes."
    Write-Host ("  2. Install baseline MSI: {0}" -f $BaselineMsi.Path)
    if ($SkipUpgrade) {
        Write-Host "  3. Upgrade step skipped because -SkipUpgrade was supplied."
    } else {
        Write-Host ("  3. Upgrade/reinstall current MSI: {0}" -f $CurrentMsi.Path)
    }
    Write-Host "  4. Verify payload under the current user's LocalAppData Programs folder."
    Write-Host "  5. Verify Start menu shortcut icon, setup options, desktop shortcut, and run-at-login state."
    if ($SkipLaunchCheck) {
        Write-Host "  6. Launch/process checks skipped because -SkipLaunchCheck was supplied."
    } elseif ($LaunchAfterInstall -ne "1") {
        Write-Host "  6. Launch/process checks skipped because DSCC_LAUNCH_AFTER_INSTALL=0."
    } else {
        Write-Host "  6. Verify one dscc-tray and one dscc-agent launch from the install folder."
    }
    if ($KeepInstalled) {
        Write-Host "  7. Uninstall skipped because -KeepInstalled was supplied."
    } else {
        Write-Host "  7. Uninstall and verify payload, shortcuts, run key, and DSCC processes are gone."
    }
    Write-Host ("  Setup properties: DSCC_START_WITH_WINDOWS={0} DSCC_CREATE_DESKTOP_SHORTCUT={1} DSCC_LAUNCH_AFTER_INSTALL={2}" -f $StartWithWindows, $CreateDesktopShortcut, $LaunchAfterInstall)
    Write-Host ("  Logs will be written under: {0}" -f $LogDirectory)
}

$repoRoot = Resolve-RepoRoot
if ([string]::IsNullOrWhiteSpace($MsiPath)) {
    $MsiPath = Resolve-DefaultMsi -RepoRoot $repoRoot
}

$currentMsi = Resolve-MsiFile -Path $MsiPath -Label "Current MSI"
if ([string]::IsNullOrWhiteSpace($BaselineMsiPath)) {
    $baselineMsi = $currentMsi
} else {
    $baselineMsi = Resolve-MsiFile -Path $BaselineMsiPath -Label "Baseline MSI"
}

if ([string]::IsNullOrWhiteSpace($LogDirectory)) {
    $LogDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("dscc-msi-smoke-{0}" -f (Get-Date -Format "yyyyMMdd-HHmmss"))
}

Write-Step "Windows installer smoke"
Write-MsiSummary -Msi $baselineMsi
if ($baselineMsi.Path -ne $currentMsi.Path) {
    Write-MsiSummary -Msi $currentMsi
} else {
    Write-Step "No -BaselineMsiPath supplied; upgrade step will reinstall the same MSI to exercise same-version upgrade behavior."
}
Write-Host ("  Install folder: {0}" -f (Get-DsccInstallFolder))
Write-Host ("  Start menu folder: {0}" -f (Get-DsccStartMenuFolder))
Write-Host ("  Desktop shortcut: {0}" -f (Get-DsccDesktopShortcut))

if (-not $Execute) {
    Write-SmokeChecklist -BaselineMsi $baselineMsi -CurrentMsi $currentMsi -LogDirectory $LogDirectory
    exit 0
}

Assert-WindowsHost
if ($TimeoutSeconds -lt 15) {
    throw "TimeoutSeconds must be at least 15."
}

New-Item -ItemType Directory -Path $LogDirectory -Force | Out-Null
Assert-CleanStart -AllowExistingInstall:$AllowExistingInstall -AllowExistingProcesses:$AllowExistingProcesses

$installProperties = @{
    DSCC_START_WITH_WINDOWS = $StartWithWindows
    DSCC_CREATE_DESKTOP_SHORTCUT = $CreateDesktopShortcut
    DSCC_LAUNCH_AFTER_INSTALL = $LaunchAfterInstall
}

Invoke-MsiAction -Action Install -Path $baselineMsi.Path -LogPath (Join-Path $LogDirectory "01-install.log") -Properties $installProperties
$installedMsi = $baselineMsi
Assert-InstalledPayload -ExpectedStartWithWindows $StartWithWindows -ExpectedDesktopShortcut $CreateDesktopShortcut
if (-not $SkipLaunchCheck -and $LaunchAfterInstall -eq "1") {
    Wait-ForDsccProcesses -Names @("dscc-tray", "dscc-agent") -TimeoutSeconds $TimeoutSeconds | Out-Null
    Assert-DsccProcessesRunFromInstallFolder
}

if (-not $SkipUpgrade) {
    Invoke-MsiAction -Action Install -Path $currentMsi.Path -LogPath (Join-Path $LogDirectory "02-upgrade.log") -Properties $installProperties
    $installedMsi = $currentMsi
    Assert-InstalledPayload -ExpectedStartWithWindows $StartWithWindows -ExpectedDesktopShortcut $CreateDesktopShortcut
    if (-not $SkipLaunchCheck -and $LaunchAfterInstall -eq "1") {
        Wait-ForDsccProcesses -Names @("dscc-tray", "dscc-agent") -TimeoutSeconds $TimeoutSeconds | Out-Null
        Assert-DsccProcessesRunFromInstallFolder
    }
} else {
    Write-Warning "Skipping upgrade step because -SkipUpgrade was supplied."
}

if (-not $KeepInstalled) {
    Invoke-MsiAction -Action Uninstall -Path $installedMsi.Path -LogPath (Join-Path $LogDirectory "03-uninstall.log")
    Assert-UninstalledPayload
    Wait-ForNoDsccProcesses -TimeoutSeconds $TimeoutSeconds
} else {
    Write-Warning "Leaving DSCC installed because -KeepInstalled was supplied."
}

Write-Step "Smoke completed successfully. Logs: $LogDirectory"
