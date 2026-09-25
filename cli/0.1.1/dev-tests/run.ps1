# Scratch-project harness for the `cli` package integration tests.
#
# `vpm test` currently compiles each test in isolation and cannot see a library
# package's own modules, so this harness assembles a temporary project that
# mounts the package as `src/cli/` and runs each `dev-tests/*.vut` as
# `src/main.vut`.
#
# Usage:
#   ./dev-tests/run.ps1 -Vpm <path-to-vpm>

param(
  [string]$Vpm = "vpm"
)

$ErrorActionPreference = "Stop"

$packageRoot = Split-Path -Parent $PSScriptRoot
$sourceRoot = Join-Path $packageRoot "src"
$testRoot = $PSScriptRoot

if (-not (Test-Path (Join-Path $sourceRoot "mod.vut"))) {
  throw "could not find package entry at $sourceRoot\mod.vut"
}

$scratch = Join-Path $env:TEMP ("vpm-cli-tests-" + [System.Guid]::NewGuid().ToString("N"))
$scratchSrc = Join-Path $scratch "src"
New-Item -ItemType Directory -Path $scratchSrc -Force | Out-Null
Copy-Item -Recurse -Path $sourceRoot -Destination (Join-Path $scratchSrc "cli")

$manifest = @"
[package]
name = "cli_tests"
version = "0.1.0"

[dependencies]
"@
Set-Content -Path (Join-Path $scratch "vpm.toml") -Value $manifest
Set-Content -Path (Join-Path $scratch "vpm.lock") -Value "lock-version = 2"

$tests = Get-ChildItem -Path $testRoot -Filter "*.vut" | Sort-Object Name
$failed = 0

Push-Location $scratch
try {
  foreach ($test in $tests) {
    Copy-Item -Path $test.FullName -Destination (Join-Path $scratchSrc "main.vut") -Force
    Write-Host ("== " + $test.Name)
    & $Vpm run
    $code = $LASTEXITCODE
    if ($code -ne 0) {
      Write-Host ("FAIL " + $test.Name + " (exit " + $code + ")")
      $failed = $failed + 1
    } else {
      Write-Host ("PASS " + $test.Name)
    }
  }
} finally {
  Pop-Location
  Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue
}

if ($failed -ne 0) {
  Write-Error ($failed.ToString() + " test(s) failed")
  exit 1
}
Write-Host "all tests passed"
