<#
.SYNOPSIS
Dump the rescue-arm9 project to text (RescueExport.java) and stamp it with the
docs/symbols.md it was made from. See docs/ghidra-setup.md, section 7.

.DESCRIPTION
Windows counterpart of export.sh. Needs GHIDRA_INSTALL_DIR (the folder with ghidraRun.bat)
and a JDK 21 (on PATH or JAVA_HOME). The export folder must be empty or missing and
outside the repository (docs/legal.md).

.EXAMPLE
.\tools\ghidra\export.ps1 -ExportDir C:\rescue-re\export -ProjectDir C:\rescue-re\ghidra
.EXAMPLE
.\tools\ghidra\export.ps1 -Check -ExportDir C:\rescue-re\export
#>
param(
    [Parameter(Mandatory = $true)][string]$ExportDir,
    [string]$ProjectDir = (Join-Path $HOME "rescue-re\ghidra"),
    [string]$Program = "arm9.bin",
    [string]$ProjectName = "rescue-arm9",
    # Exit 0 if the export was made from the current docs/symbols.md, 1 if stale or unfinished.
    [switch]$Check
)
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$symbols = Join-Path $repoRoot "docs\symbols.md"

function Get-Sha1([string]$path) {
    (Get-FileHash -Algorithm SHA1 -LiteralPath $path).Hash.ToLowerInvariant()
}

$out = [System.IO.Path]::GetFullPath($ExportDir)
if ($Check) {
    $stamp = Join-Path $out "STAMP"
    if (-not (Test-Path $stamp)) { Write-Output "stale: no finished export in $out (no STAMP)"; exit 1 }
    # A STAMP from before this wrapper has no sha1 line: that counts as stale, not an error.
    $match = Select-String -Path $stamp -Pattern '^symbols\.md-sha1: (\S+)' | Select-Object -First 1
    if ($match -and $match.Matches[0].Groups[1].Value -eq (Get-Sha1 $symbols)) {
        Write-Output "fresh: $out was exported from the current docs/symbols.md"
        exit 0
    }
    Write-Output "stale: docs/symbols.md changed since the export in $out; rerun tools\ghidra\export.ps1"
    exit 1
}

if (-not $env:GHIDRA_INSTALL_DIR) { throw "set GHIDRA_INSTALL_DIR to your Ghidra install folder" }
$headless = Join-Path $env:GHIDRA_INSTALL_DIR "support\analyzeHeadless.bat"
if (-not (Test-Path $headless)) { throw "$headless not found" }

if ((Test-Path $out) -and @(Get-ChildItem -Force -LiteralPath $out).Count -gt 0) {
    throw "$out is not empty; remove it first so no stale file survives"
}
New-Item -ItemType Directory -Force -Path $out | Out-Null

# Read the symbols before the run: an edit during the export is then reported as stale.
$symbolsSha = Get-Sha1 $symbols
$version = (Select-String -Path (Join-Path $env:GHIDRA_INSTALL_DIR "Ghidra\application.properties") -Pattern '^application\.version=(.+)$' | Select-Object -First 1).Matches.Groups[1].Value
$log = "$out.log"
$start = Get-Date
& $headless (Resolve-Path $ProjectDir).Path $ProjectName -process $Program -noanalysis -readOnly `
    -scriptPath (Join-Path $PSScriptRoot "") -postScript RescueExport.java $out *> $log
$status = $LASTEXITCODE
$elapsed = [int]((Get-Date) - $start).TotalSeconds

# analyzeHeadless can exit 0 when a script fails, so the STAMP (written last) is the proof.
if ($status -ne 0 -or -not (Test-Path (Join-Path $out "STAMP")) -or -not (Test-Path (Join-Path $out "FAILED"))) {
    throw "export did not finish (exit $status); see $log"
}
Add-Content -Path (Join-Path $out "STAMP") -Value @(
    "symbols.md-sha1: $symbolsSha",
    "ghidra: $(if ($version) { $version } else { 'unknown' })",
    "elapsed-seconds: $elapsed")
Get-Content (Join-Path $out "STAMP")
$failed = @(Get-Content (Join-Path $out "FAILED")).Count
Write-Output "failed functions: $failed (listed in $(Join-Path $out 'FAILED'))"
Write-Output "OK: $out (elapsed ${elapsed}s)"
