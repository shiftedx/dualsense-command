$ErrorActionPreference = 'Stop'
$packaging = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'package-msi.ps1') -Raw
if ($packaging -notmatch '<Custom Action="LaunchTrayAfterInstall" After="BackupPersistedState">NOT REMOVE AND DSCC_LAUNCH_AFTER_INSTALL = "1"</Custom>') {
    throw 'Install, repair, and upgrade must honor requested launch; uninstall must not launch.'
}
foreach ($command in @('dscc-tray.exe --stop')) {
    if (-not $packaging.Contains('ExeCommand="' + $command + '"')) {
        throw 'Embedded EXE actions must preserve argv[0] before the first option.'
    }
}
$tokens = $null
$errors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile(
    (Join-Path $PSScriptRoot 'windows-installer-smoke.ps1'), [ref]$tokens, [ref]$errors)
if ($errors.Count) { throw ($errors | Out-String) }
$names = @('Resolve-MsiFile', 'Assert-RetainedConfigProbe')
foreach ($function in $ast.FindAll({ param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] }, $true)) {
    if ($function.Name -in $names) { Invoke-Expression $function.Extent.Text }
}
function Expect-Failure([scriptblock]$Action) {
    $failed = $false
    try { & $Action | Out-Null } catch { $failed = $true }
    if (-not $failed) { throw 'Expected invalid preflight evidence to fail.' }
}
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('dscc-smoke-unit-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot | Out-Null
try {
    $msi = Join-Path $testRoot 'candidate.msi'
    New-Item -ItemType File -Path $msi | Out-Null
    Expect-Failure { Resolve-MsiFile -Path $msi -Label 'Candidate' }
    Set-Content -LiteralPath $msi -Value 'preflight fixture'
    $summary = Resolve-MsiFile -Path $msi -Label 'Candidate'
    if ($summary.Sha256 -ne (Get-FileHash -LiteralPath $msi).Hash.ToLowerInvariant()) { throw 'Incorrect artifact identity.' }
    $probe = Join-Path $testRoot 'config-probe.txt'
    Set-Content -LiteralPath $probe -Value 'preserved settings'
    $hash = (Get-FileHash -LiteralPath $probe).Hash
    Assert-RetainedConfigProbe -Path $probe -ExpectedHash $hash
    Set-Content -LiteralPath $probe -Value 'changed settings'
    Expect-Failure { Assert-RetainedConfigProbe -Path $probe -ExpectedHash $hash }
    Remove-Item -LiteralPath $probe
    Expect-Failure { Assert-RetainedConfigProbe -Path $probe -ExpectedHash $hash }
    Write-Output 'Installer preflight and retention regression checks pass.'
} finally {
    $resolved = [IO.Path]::GetFullPath($testRoot)
    if (-not $resolved.StartsWith([IO.Path]::GetFullPath([IO.Path]::GetTempPath()), [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe test cleanup path.' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
