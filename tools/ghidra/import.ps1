<#
.SYNOPSIS
Build a Ghidra project with Blue's ARM9 and all ARM9 overlays mapped at their real
addresses, from the output of tools/nds_extract_code.py. See docs/ghidra-setup.md.

.DESCRIPTION
Windows counterpart of import.sh. With -Arm7, imports arm7.bin as a second program. Needs GHIDRA_INSTALL_DIR (the folder with
ghidraRun.bat) and a JDK 21 (on PATH or JAVA_HOME). Never commit the project or the
extracted binaries (docs/legal.md).

.EXAMPLE
.\tools\ghidra\import.ps1 -CodeDir C:\rescue-re\code -ProjectDir C:\rescue-re\ghidra -Check ov_0001+0x72688
#>
param(
    [Parameter(Mandatory = $true)][string]$CodeDir,
    [Parameter(Mandatory = $true)][string]$ProjectDir,
    [string]$ProjectName = "rescue-arm9",
    [switch]$NoAnalysis,
    # Import arm7.bin as the program arm7.bin (ARMv4T) instead of the ARM9. Needs "arm7" in manifest.json.
    [switch]$Arm7,
    # Addresses to print references to after analysis, e.g. ov_0001+0x72688.
    [string[]]$Check = @()
)
$ErrorActionPreference = "Stop"

if (-not $env:GHIDRA_INSTALL_DIR) { throw "set GHIDRA_INSTALL_DIR to your Ghidra install folder" }
$headless = Join-Path $env:GHIDRA_INSTALL_DIR "support\analyzeHeadless.bat"
if (-not $IsWindows -and $null -ne $IsWindows) {
    $headless = Join-Path $env:GHIDRA_INSTALL_DIR "support/analyzeHeadless"
}
if (-not (Test-Path $headless)) { throw "$headless not found" }
$manifest = Join-Path $CodeDir "manifest.json"
if (-not (Test-Path $manifest)) { throw "$manifest not found; run tools/nds_extract_code.py first" }

$scriptDir = $PSScriptRoot
$repoRoot = (Resolve-Path (Join-Path $scriptDir "..\..")).Path
$codeDirAbs = (Resolve-Path $CodeDir).Path
$projAbs = [System.IO.Path]::GetFullPath($ProjectDir)

# Refuse a project inside the repository unless git ignores it (docs/legal.md).
$sep = [System.IO.Path]::DirectorySeparatorChar
if (($projAbs + $sep).StartsWith($repoRoot + $sep, [System.StringComparison]::OrdinalIgnoreCase)) {
    & git -C $repoRoot check-ignore -q (Join-Path $projAbs "x.gpr") 2>$null
    if ($LASTEXITCODE -ne 0) { throw "$projAbs is inside the repository and not git-ignored (docs/legal.md)" }
}
New-Item -ItemType Directory -Force -Path $projAbs | Out-Null

if ($Arm7) {
    $arm7 = (Get-Content $manifest -Raw | ConvertFrom-Json).arm7
    if (-not $arm7) { throw "manifest has no arm7 entry (the ROM has no ARM7 binary)" }
    if ($null -eq $arm7.autoloads) { throw "manifest predates the ARM7 layout; run tools/nds_extract_code.py again" }
    $log7 = Join-Path $projAbs "$ProjectName-arm7-import.log"
    $args7 = @(
        $projAbs, $ProjectName,
        "-import", (Join-Path $codeDirAbs "arm7.bin"), "-overwrite",
        "-loader", "BinaryLoader", "-loader-baseAddr", $arm7.ram_address,
        "-processor", "ARM:LE:32:v4t", "-cspec", "default",
        "-scriptPath", $scriptDir,
        "-preScript", "RescueNdsImport.java", (Join-Path $codeDirAbs "manifest.json"), "arm7",
        "-preScript", "RescueSharedRam.java",
        "-postScript", "RescueNdsReport.java") + $Check
    if ($NoAnalysis) { $args7 += "-noanalysis" }
    & $headless @args7 *> $log7
    $status7 = $LASTEXITCODE
    # analyzeHeadless exits 0 even when a script fails, so check the log as well.
    $text7 = Get-Content $log7 -Raw
    $text7 -split "`n" | Select-String -Pattern 'RESCUE(-REPORT)?:' | ForEach-Object { $_.Line.Trim() }
    if ($status7 -ne 0 -or $text7 -match 'SCRIPT ERROR|RESCUE: FAILED' -or $text7 -notmatch 'RESCUE: mapped arm7') {
        Write-Error "ARM7 import failed; see $log7"
        exit 1
    }
    Write-Host "OK: program arm7.bin in $(Join-Path $projAbs "$ProjectName.gpr") (ARM9 program unchanged)"
    exit 0
}

$log = Join-Path $projAbs "$ProjectName-import.log"
$argsList = @(
    $projAbs, $ProjectName,
    "-import", (Join-Path $codeDirAbs "arm9.bin"), "-overwrite",
    "-loader", "BinaryLoader", "-loader-baseAddr", "0x02000000",
    "-processor", "ARM:LE:32:v5t", "-cspec", "default",
    "-scriptPath", $scriptDir,
    "-preScript", "RescueNdsImport.java", (Join-Path $codeDirAbs "manifest.json"),
    "-preScript", "RescueSharedRam.java",
    "-postScript", "RescueNdsReport.java"
) + $Check
if ($NoAnalysis) { $argsList += "-noanalysis" }

Write-Host "Running analyzeHeadless; log: $log"
& $headless @argsList *> $log
$status = $LASTEXITCODE

$lines = Get-Content $log
$lines | Select-String -Pattern 'RESCUE(-REPORT)?:' | ForEach-Object {
    ($_.Line -replace '^.*?(RESCUE(-REPORT)?:)', '$1') -replace ' \((GhidraScript|HeadlessAnalyzer)\)\s*$', ''
}
# analyzeHeadless exits 0 even when a script fails, so check the log as well.
$failed = ($status -ne 0) -or
    ($lines | Select-String -Pattern 'SCRIPT ERROR|RESCUE: FAILED' -Quiet) -or
    -not ($lines | Select-String -Pattern 'RESCUE: mapped' -Quiet)
if ($failed) {
    Write-Error "import failed (exit $status); see $log"
    exit 1
}
Write-Host "OK: project $(Join-Path $projAbs "$ProjectName.gpr")"
