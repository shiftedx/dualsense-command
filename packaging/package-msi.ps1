param(
    [string]$Version = "0.2.8",
    [string]$TargetTriple,
    [switch]$SkipWebBuild,
    [switch]$AllowDebugAgent,
    [string]$CertificatePath,
    [string]$CertificatePassword,
    [string]$TimestampUrl = 'http://timestamp.digicert.com'
)

$ErrorActionPreference = "Stop"

function Resolve-RepoRoot {
    $scriptDir = Split-Path -Parent $PSCommandPath
    return (Resolve-Path (Join-Path $scriptDir "..")).Path
}

function Escape-Xml([string]$Value) {
    return [System.Security.SecurityElement]::Escape($Value)
}

function New-SafeId([string]$Prefix, [int]$Index) {
    return "{0}_{1}" -f $Prefix, $Index
}

function New-ComponentGuid([string]$Seed) {
    $md5 = [System.Security.Cryptography.MD5]::Create()
    try {
        $hash = $md5.ComputeHash([System.Text.Encoding]::UTF8.GetBytes("DualSenseCommandCenter|$Seed"))
        $guidBytes = New-Object byte[] 16
        [Array]::Copy($hash, $guidBytes, 16)
        return "{0}{1}{2}" -f "{", ([Guid]::new($guidBytes)).ToString().ToUpperInvariant(), "}"
    } finally {
        $md5.Dispose()
    }
}

function Add-TextFile([string]$Path, [string]$Content) {
    $dir = Split-Path -Parent $Path
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir | Out-Null
    }
    Set-Content -LiteralPath $Path -Value $Content -Encoding ASCII
}

function Copy-DirectoryClean([string]$Source, [string]$Destination) {
    if (Test-Path $Destination) {
        Remove-Item -LiteralPath $Destination -Recurse -Force
    }
    New-Item -ItemType Directory -Path $Destination | Out-Null
    Copy-Item -Path (Join-Path $Source "*") -Destination $Destination -Recurse -Force
}

function Assert-FileSha256([string]$Path, [string]$ExpectedHash) {
    if (-not (Test-Path -LiteralPath $Path)) {
        throw "Expected file does not exist: $Path"
    }
    $actualHash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant()
    if ($actualHash -ne $ExpectedHash.ToUpperInvariant()) {
        throw "Hash mismatch for $Path. Expected $ExpectedHash but found $actualHash."
    }
}

function Ensure-Wix3([string]$TargetRoot) {
    $toolDir = Join-Path $TargetRoot "tools\wix314"
    $candle = Join-Path $toolDir "candle.exe"
    $light = Join-Path $toolDir "light.exe"
    $wixSha256 = "6AC824E1642D6F7277D0ED7EA09411A508F6116BA6FAE0AA5F2C7DAA2FF43D31"

    if ((Test-Path $candle) -and (Test-Path $light)) {
        return @{
            Candle = $candle
            Light = $light
        }
    }

    $toolsRoot = Join-Path $TargetRoot "tools"
    $zipPath = Join-Path $toolsRoot "wix314-binaries.zip"
    New-Item -ItemType Directory -Path $toolsRoot -Force | Out-Null

    if (-not (Test-Path $zipPath)) {
        $wixUrl = "https://github.com/wixtoolset/wix3/releases/download/wix3141rtm/wix314-binaries.zip"
        Write-Output "Downloading WiX Toolset v3.14.1 portable binaries..."
        Invoke-WebRequest -Uri $wixUrl -OutFile $zipPath
    }
    Assert-FileSha256 -Path $zipPath -ExpectedHash $wixSha256

    if (Test-Path $toolDir) {
        Remove-Item -LiteralPath $toolDir -Recurse -Force
    }
    New-Item -ItemType Directory -Path $toolDir -Force | Out-Null
    Expand-Archive -LiteralPath $zipPath -DestinationPath $toolDir -Force

    if (-not ((Test-Path $candle) -and (Test-Path $light))) {
        throw "WiX portable binaries did not extract correctly."
    }

    return @{
        Candle = $candle
        Light = $light
    }
}

function Find-SignTool {
    # Search common Windows 10/11 SDK locations for the newest x64 signtool.exe.
    $candidates = @()
    $sdkRoots = @(
        (Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'),
        (Join-Path $env:ProgramFiles 'Windows Kits\10\bin')
    )
    foreach ($root in $sdkRoots) {
        if ([string]::IsNullOrWhiteSpace($root)) { continue }
        if (-not (Test-Path -LiteralPath $root)) { continue }
        $candidates += Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue |
            ForEach-Object {
                $exe = Join-Path $_.FullName 'x64\signtool.exe'
                if (Test-Path -LiteralPath $exe) {
                    [pscustomobject]@{ Version = $_.Name; Path = $exe }
                }
            }
    }

    if ($candidates.Count -eq 0) {
        # Fall back to PATH lookup.
        $onPath = Get-Command signtool.exe -ErrorAction SilentlyContinue
        if ($onPath) { return $onPath.Source }
        return $null
    }

    # Sort by version-folder name (lexicographic on the 10.0.x.0 strings works well enough);
    # try a real System.Version parse first and fall back to the string sort.
    $sorted = $candidates | Sort-Object -Property @{
        Expression = {
            try { [System.Version]::Parse($_.Version) } catch { [System.Version]::new(0, 0) }
        }
    }, Version -Descending
    return $sorted[0].Path
}

function Invoke-Signtool {
    param(
        [Parameter(Mandatory = $true)][string]$SignTool,
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string]$CertificatePath,
        [Parameter(Mandatory = $true)][string]$CertificatePassword,
        [Parameter(Mandatory = $true)][string]$TimestampUrl,
        [string]$Description = 'DualSense Command Center'
    )
    & $SignTool sign /f $CertificatePath /p $CertificatePassword /tr $TimestampUrl /td sha256 /fd sha256 /d $Description $FilePath
    if ($LASTEXITCODE -ne 0) {
        throw "signtool failed for '$FilePath' with exit code $LASTEXITCODE."
    }
}

function Write-DirectoryXml {
    param(
        [string]$DirectoryPath,
        [string]$DirectoryId,
        [int]$Indent,
        [ref]$NextId,
        [System.Collections.Generic.List[string]]$ComponentRefs,
        [System.Collections.Generic.List[string]]$Lines
    )

    $pad = " " * $Indent
    $cleanupComponentId = New-SafeId "RemoveFolderComponent" $NextId.Value
    $cleanupId = New-SafeId "RemoveFolder" $NextId.Value
    $cleanupGuid = New-ComponentGuid ("folder:{0}" -f $DirectoryId)
    $NextId.Value += 1
    $ComponentRefs.Add(("      <ComponentRef Id=""{0}"" />" -f $cleanupComponentId))
    $Lines.Add(('{0}<Component Id="{1}" Guid="{2}">' -f $pad, $cleanupComponentId, $cleanupGuid))
    $Lines.Add(('{0}  <RemoveFolder Id="{1}" Directory="{2}" On="uninstall" />' -f $pad, $cleanupId, $DirectoryId))
    $Lines.Add(('{0}  <RegistryValue Root="HKCU" Key="Software\DualSenseCommand\DualSenseCommandCenter\Folders" Name="{1}" Type="integer" Value="1" KeyPath="yes" />' -f $pad, $cleanupComponentId))
    $Lines.Add(('{0}</Component>' -f $pad))

    foreach ($file in Get-ChildItem -LiteralPath $DirectoryPath -File | Sort-Object Name) {
        if ($DirectoryId -eq "INSTALLFOLDER" -and $file.Name -eq "dscc-tray.exe") {
            $componentId = "TrayExeComponent"
            $fileId = "TrayExe"
        } elseif ($DirectoryId -eq "INSTALLFOLDER" -and $file.Name -eq "dscc-agent.exe") {
            $componentId = "AgentExeComponent"
            $fileId = "AgentExe"
        } else {
            $componentId = New-SafeId "Component" $NextId.Value
            $fileId = New-SafeId "File" $NextId.Value
        }
        $componentGuid = New-ComponentGuid ("file:{0}:{1}" -f $DirectoryId, $file.Name)
        $NextId.Value += 1
        $ComponentRefs.Add(("      <ComponentRef Id=""{0}"" />" -f $componentId))
        $Lines.Add(('{0}<Component Id="{1}" Guid="{2}">' -f $pad, $componentId, $componentGuid))
        $Lines.Add(('{0}  <File Id="{1}" Source="{2}" />' -f $pad, $fileId, (Escape-Xml $file.FullName)))
        $Lines.Add(('{0}  <RegistryValue Root="HKCU" Key="Software\DualSenseCommand\DualSenseCommandCenter\Files" Name="{1}" Type="integer" Value="1" KeyPath="yes" />' -f $pad, $componentId))
        $Lines.Add(('{0}</Component>' -f $pad))
    }

    foreach ($dir in Get-ChildItem -LiteralPath $DirectoryPath -Directory | Sort-Object Name) {
        $childDirId = New-SafeId "Dir" $NextId.Value
        $NextId.Value += 1
        $Lines.Add(('{0}<Directory Id="{1}" Name="{2}">' -f $pad, $childDirId, (Escape-Xml $dir.Name)))
        Write-DirectoryXml -DirectoryPath $dir.FullName -DirectoryId $childDirId -Indent ($Indent + 2) -NextId $NextId -ComponentRefs $ComponentRefs -Lines $Lines
        $Lines.Add(('{0}</Directory>' -f $pad))
    }
}

$repoRoot = Resolve-RepoRoot
$webRoot = Join-Path $repoRoot "web"
$targetRoot = Join-Path $repoRoot "target"
$stagingRoot = Join-Path $targetRoot "installer\staging"
$wixRoot = Join-Path $targetRoot "installer\wix"
$msiRoot = Join-Path $targetRoot "installer"
$msiPath = Join-Path $msiRoot ("DualSenseCommandCenter-{0}.msi" -f $Version)

if (-not $SkipWebBuild) {
    Push-Location $webRoot
    try {
        & npm.cmd run build
    } finally {
        Pop-Location
    }
}

if ([string]::IsNullOrWhiteSpace($TargetTriple)) {
    $buildRoot = $targetRoot
} else {
    $buildRoot = Join-Path $targetRoot $TargetTriple
}

$releaseAgent = Join-Path $buildRoot "release\dscc-agent.exe"
$debugAgent = Join-Path $buildRoot "debug\dscc-agent.exe"
$releaseTray = Join-Path $buildRoot "release\dscc-tray.exe"
$debugTray = Join-Path $buildRoot "debug\dscc-tray.exe"
if (Test-Path $releaseAgent) {
    $agentExe = $releaseAgent
} elseif ($AllowDebugAgent -and (Test-Path $debugAgent)) {
    $agentExe = $debugAgent
} else {
    $targetHint = if ([string]::IsNullOrWhiteSpace($TargetTriple)) { "" } else { " --target $TargetTriple" }
    throw "No release dscc-agent.exe found. Build with cargo build -p dscc-agent --release$targetHint, or pass -AllowDebugAgent for a local test MSI."
}
if (Test-Path $releaseTray) {
    $trayExe = $releaseTray
} elseif ($AllowDebugAgent -and (Test-Path $debugTray)) {
    $trayExe = $debugTray
} else {
    $targetHint = if ([string]::IsNullOrWhiteSpace($TargetTriple)) { "" } else { " --target $TargetTriple" }
    throw "No release dscc-tray.exe found. Build with cargo build -p dscc-tray --release$targetHint, or pass -AllowDebugAgent for a local test MSI."
}

$webDist = Join-Path $webRoot "dist"
if (-not (Test-Path (Join-Path $webDist "index.html"))) {
    throw "web/dist is missing. Run npm run build first."
}

if (Test-Path $stagingRoot) {
    Remove-Item -LiteralPath $stagingRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $stagingRoot | Out-Null
New-Item -ItemType Directory -Path $wixRoot -Force | Out-Null

Copy-Item -LiteralPath $agentExe -Destination (Join-Path $stagingRoot "dscc-agent.exe") -Force
Copy-Item -LiteralPath $trayExe -Destination (Join-Path $stagingRoot "dscc-tray.exe") -Force
Copy-DirectoryClean -Source $webDist -Destination (Join-Path $stagingRoot "web\dist")

# Resolve signtool once if signing was requested, and prompt for the password if it
# wasn't supplied. Sign the staged binaries here (before WiX harvests them) so the
# bundled .exe files inside the MSI carry the signature too.
$signTool = $null
if (-not [string]::IsNullOrWhiteSpace($CertificatePath)) {
    if (-not (Test-Path -LiteralPath $CertificatePath)) {
        throw "CertificatePath '$CertificatePath' does not exist."
    }
    if ([string]::IsNullOrEmpty($CertificatePassword)) {
        $secure = Read-Host -AsSecureString -Prompt "Password for $CertificatePath"
        $bstr = [System.Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
        try {
            $CertificatePassword = [System.Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr)
        } finally {
            [System.Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr)
        }
    }
    $signTool = Find-SignTool
    if (-not $signTool) {
        throw "Code signing requested but signtool.exe could not be located. Install the Windows 10/11 SDK or add signtool.exe to PATH."
    }

    foreach ($staged in @((Join-Path $stagingRoot 'dscc-agent.exe'), (Join-Path $stagingRoot 'dscc-tray.exe'))) {
        Invoke-Signtool -SignTool $signTool -FilePath $staged -CertificatePath $CertificatePath -CertificatePassword $CertificatePassword -TimestampUrl $TimestampUrl
    }
}

$stopScript = @"
@echo off
"%SystemRoot%\System32\taskkill.exe" /IM dscc-agent.exe /F /T >nul 2>nul
"%SystemRoot%\System32\taskkill.exe" /IM dscc-tray.exe /F /T >nul 2>nul
"@

$backupStateScript = @"
@echo off
setlocal
set "DSCC_VERSION=%~1"
if "%DSCC_VERSION%"=="" set "DSCC_VERSION=unknown"

call :BackupState "%APPDATA%\DualSenseCommand\DualSenseCommandCenter\config"

if not "%DSCC_CONFIG_DIR%"=="" (
    call :BackupState "%DSCC_CONFIG_DIR%"
)

exit /b 0

:BackupState
set "CONFIG_DIR=%~1"
if "%CONFIG_DIR%"=="" exit /b 0
set "STATE_FILE=%CONFIG_DIR%\state.json"
if not exist "%STATE_FILE%" exit /b 0
set "BACKUP_FILE=%CONFIG_DIR%\state.preinstall-%DSCC_VERSION%.json"
if exist "%BACKUP_FILE%" (
    set "BACKUP_FILE=%CONFIG_DIR%\state.preinstall-%DSCC_VERSION%-%RANDOM%.json"
)
copy /Y "%STATE_FILE%" "%BACKUP_FILE%" >nul 2>nul
exit /b 0
"@

$readme = @"
DualSense Command Center test build

1. Install the MSI.
2. Open "DualSense Command Center" from the Start menu.
3. The tray icon starts the local agent and opens the UI.
4. Right-click the tray icon to open the UI, restart DSCC, stop DSCC, or quit.
5. The login startup entry starts the tray and agent silently; opening the Start menu shortcut brings up the UI.
6. Hardware output is enabled by default when the agent starts.
7. For Forza testing, enable Data Out in-game and point it at 127.0.0.1 port 5300.
8. The local UI opens at http://127.0.0.1:43473/.
9. During install/upgrade, DSCC backs up persisted user state to state.preinstall-$Version.json when state.json exists.
"@

Add-TextFile -Path (Join-Path $stagingRoot "Stop DSCC.cmd") -Content $stopScript
Add-TextFile -Path (Join-Path $stagingRoot "Backup DSCC State.cmd") -Content $backupStateScript
Add-TextFile -Path (Join-Path $stagingRoot "README_TESTING.txt") -Content $readme

$componentRefs = [System.Collections.Generic.List[string]]::new()
$directoryLines = [System.Collections.Generic.List[string]]::new()
$nextId = 1
$nextIdRef = [ref]$nextId
Write-DirectoryXml -DirectoryPath $stagingRoot -DirectoryId "INSTALLFOLDER" -Indent 8 -NextId $nextIdRef -ComponentRefs $componentRefs -Lines $directoryLines

$shortcutComponentId = "StartMenuShortcuts"
$componentRefs.Add(('      <ComponentRef Id="{0}" />' -f $shortcutComponentId))
$localProgramsCleanupComponentId = "LocalProgramsFolderCleanup"
$componentRefs.Add(('      <ComponentRef Id="{0}" />' -f $localProgramsCleanupComponentId))
$runAtLoginComponentId = "RunAtLogin"
$componentRefs.Add(('      <ComponentRef Id="{0}" />' -f $runAtLoginComponentId))
$componentRefText = $componentRefs -join "`r`n"
$directoryText = $directoryLines -join "`r`n"

$wxs = @"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="DualSense Command Center" Language="1033" Version="$Version" Manufacturer="DualSense Command" UpgradeCode="{7D3E3504-865B-4A72-A61B-86C977729589}">
    <Package InstallerVersion="500" Compressed="yes" InstallScope="perUser" />
    <MajorUpgrade AllowSameVersionUpgrades="yes" DowngradeErrorMessage="A newer version of DualSense Command Center is already installed." />
    <MediaTemplate EmbedCab="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="LocalAppDataFolder">
        <Directory Id="LocalProgramsFolder" Name="Programs">
          <Directory Id="INSTALLFOLDER" Name="DualSense Command Center">
$directoryText
          </Directory>
        </Directory>
      </Directory>
      <Directory Id="ProgramMenuFolder">
        <Directory Id="ApplicationProgramsFolder" Name="DualSense Command Center" />
      </Directory>
    </Directory>

    <DirectoryRef Id="ApplicationProgramsFolder">
      <Component Id="$shortcutComponentId" Guid="{22BE0FA0-2187-4F88-95EF-D0A1BEB53D88}">
        <Shortcut Id="StartMenuShortcut" Name="DualSense Command Center" Target="[INSTALLFOLDER]dscc-tray.exe" WorkingDirectory="INSTALLFOLDER" />
        <Shortcut Id="StartMenuStopShortcut" Name="Stop DualSense Command Center" Target="[INSTALLFOLDER]Stop DSCC.cmd" WorkingDirectory="INSTALLFOLDER" />
        <RemoveFolder Id="ApplicationProgramsFolder" On="uninstall" />
        <RegistryValue Root="HKCU" Key="Software\DualSenseCommand\DualSenseCommandCenter" Name="installed" Type="integer" Value="1" KeyPath="yes" />
      </Component>
    </DirectoryRef>

    <DirectoryRef Id="INSTALLFOLDER">
      <Component Id="$runAtLoginComponentId" Guid="{CF89093D-7604-455F-8E51-5929396D60B1}">
        <RegistryValue Root="HKCU" Key="Software\Microsoft\Windows\CurrentVersion\Run" Name="DualSense Command Center" Type="string" Value="&quot;[INSTALLFOLDER]dscc-tray.exe&quot; --startup" KeyPath="yes" />
      </Component>
    </DirectoryRef>

    <DirectoryRef Id="LocalProgramsFolder">
      <Component Id="$localProgramsCleanupComponentId" Guid="{0F18B823-1C32-4A6C-8D29-3137E10DA9B0}">
        <RemoveFolder Id="RemoveLocalProgramsFolder" Directory="LocalProgramsFolder" On="uninstall" />
        <RegistryValue Root="HKCU" Key="Software\DualSenseCommand\DualSenseCommandCenter\Folders" Name="LocalProgramsFolder" Type="integer" Value="1" KeyPath="yes" />
      </Component>
    </DirectoryRef>

    <Feature Id="MainFeature" Title="DualSense Command Center" Level="1">
$componentRefText
    </Feature>

    <CustomAction Id="StopExistingAgent" Directory="TARGETDIR" ExeCommand="&quot;[SystemFolder]taskkill.exe&quot; /IM dscc-agent.exe /F /T" Execute="immediate" Return="ignore" Impersonate="yes" />
    <CustomAction Id="StopExistingTray" Directory="TARGETDIR" ExeCommand="&quot;[SystemFolder]taskkill.exe&quot; /IM dscc-tray.exe /F /T" Execute="immediate" Return="ignore" Impersonate="yes" />
    <CustomAction Id="BackupPersistedState" Directory="INSTALLFOLDER" ExeCommand="&quot;[INSTALLFOLDER]Backup DSCC State.cmd&quot; &quot;$Version&quot;" Execute="immediate" Return="ignore" Impersonate="yes" />
    <CustomAction Id="LaunchTrayAfterInstall" Directory="INSTALLFOLDER" ExeCommand="&quot;[INSTALLFOLDER]dscc-tray.exe&quot;" Return="asyncNoWait" Impersonate="yes" />
    <InstallExecuteSequence>
      <Custom Action="StopExistingAgent" Before="InstallValidate">NOT REMOVE</Custom>
      <Custom Action="StopExistingTray" After="StopExistingAgent">NOT REMOVE</Custom>
      <Custom Action="BackupPersistedState" After="InstallFinalize">NOT REMOVE</Custom>
      <Custom Action="LaunchTrayAfterInstall" After="BackupPersistedState">NOT Installed</Custom>
    </InstallExecuteSequence>
  </Product>
</Wix>
"@

$wxsPath = Join-Path $wixRoot "DualSenseCommandCenter.wxs"
Add-TextFile -Path $wxsPath -Content $wxs

$wixTools = Ensure-Wix3 -TargetRoot $targetRoot
$wixObjPath = Join-Path $wixRoot "DualSenseCommandCenter.wixobj"
& $wixTools.Candle -nologo -arch x64 -out $wixObjPath $wxsPath
if ($LASTEXITCODE -ne 0) {
    throw "WiX candle failed with exit code $LASTEXITCODE."
}
& $wixTools.Light -nologo -spdb -sice:ICE61 -sice:ICE91 -out $msiPath $wixObjPath
if ($LASTEXITCODE -ne 0) {
    throw "WiX light failed with exit code $LASTEXITCODE."
}

if ($signTool) {
    Invoke-Signtool -SignTool $signTool -FilePath $msiPath -CertificatePath $CertificatePath -CertificatePassword $CertificatePassword -TimestampUrl $TimestampUrl
    $certSubject = try {
        (Get-PfxCertificate -FilePath $CertificatePath -ErrorAction Stop).Subject
    } catch {
        '<unknown subject>'
    }
    Write-Output "MSI signed with $certSubject"
} else {
    Write-Output "MSI built unsigned (no -CertificatePath supplied)"
}

Write-Output "MSI: $msiPath"
Write-Output "Agent: $agentExe"
Write-Output "Tray: $trayExe"
Write-Output "Staging: $stagingRoot"
