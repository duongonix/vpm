# Builds the hello-cli dogfood example against the package and runs a matrix of
# real invocations, asserting stdout and exit codes.
#
# Usage:
#   ./examples/run.ps1 -Vpm <path-to-vpm>

param(
  [string]$Vpm = "vpm"
)

$ErrorActionPreference = "Continue"

$packageRoot = Split-Path -Parent $PSScriptRoot
$sourceRoot = Join-Path $packageRoot "src"
$example = Join-Path $PSScriptRoot "hello_cli.vut"

if (-not (Test-Path (Join-Path $sourceRoot "mod.vut"))) {
  throw "could not find package entry at $sourceRoot\mod.vut"
}

$scratch = Join-Path $env:TEMP ("vpm-cli-example-" + [System.Guid]::NewGuid().ToString("N"))
$scratchSrc = Join-Path $scratch "src"
New-Item -ItemType Directory -Path $scratchSrc -Force | Out-Null
Copy-Item -Recurse -Path $sourceRoot -Destination (Join-Path $scratchSrc "cli")
Copy-Item -Path $example -Destination (Join-Path $scratchSrc "main.vut")

$manifest = @"
[package]
name = "hello_cli_example"
version = "0.1.0"

[dependencies]
"@
Set-Content -Path (Join-Path $scratch "vpm.toml") -Value $manifest
Set-Content -Path (Join-Path $scratch "vpm.lock") -Value "lock-version = 2"

$failed = 0

function Check-Contains([string]$label, [string]$text, [string]$needle) {
  if ($text.Contains($needle)) {
    Write-Host "PASS $label"
  } else {
    Write-Host "FAIL $label (missing '$needle')"
    $script:failed = $script:failed + 1
  }
}

function Check-Absent([string]$label, [string]$text, [string]$needle) {
  if (-not $text.Contains($needle)) {
    Write-Host "PASS $label"
  } else {
    Write-Host "FAIL $label (unexpected '$needle')"
    $script:failed = $script:failed + 1
  }
}

function Check-Code([string]$label, [int]$actual, [int]$expected) {
  if ($actual -eq $expected) {
    Write-Host "PASS $label"
  } else {
    Write-Host "FAIL $label (exit $actual, want $expected)"
    $script:failed = $script:failed + 1
  }
}

Push-Location $scratch
try {
  # Warm the build once.
  & $Vpm build | Out-Null

  $out = (& $Vpm run -- --version 2>&1 | Out-String)
  Check-Contains "version.text" $out "hello-cli 1.0.0"
  Check-Code "version.code" $LASTEXITCODE 0

  $out = (& $Vpm run -- --help 2>&1 | Out-String)
  Check-Contains "help.usage" $out "usage: hello-cli <command> [options]"
  Check-Contains "help.init" $out "init"
  Check-Contains "help.config" $out "config"
  Check-Code "help.code" $LASTEXITCODE 0

  $out = (& $Vpm run -- build --release --target wasm 2>&1 | Out-String)
  Check-Contains "build.release" $out "build release target=wasm"
  Check-Code "build.code" $LASTEXITCODE 0

  $out = (& $Vpm run -- b -r 2>&1 | Out-String)
  Check-Contains "alias.flag" $out "build release target=host"
  Check-Code "alias.code" $LASTEXITCODE 0

  $out = (& $Vpm run -- run --port 9000 2>&1 | Out-String)
  Check-Contains "run.port" $out "run port=9000"
  Check-Code "run.code" $LASTEXITCODE 0

  $out = (& $Vpm run -- config theme --value dark 2>&1 | Out-String)
  Check-Contains "config.value" $out "config theme=dark"
  Check-Code "config.code" $LASTEXITCODE 0

  $out = (& $Vpm run -- config --help 2>&1 | Out-String)
  Check-Contains "command.help" $out "usage: hello-cli config"
  Check-Contains "command.help.arg" $out "<key>"
  Check-Code "command.help.code" $LASTEXITCODE 0

  $out = (& $Vpm run -- nope 2>&1 | Out-String)
  Check-Contains "unknown.message" $out "unknown command"
  Check-Code "unknown.code" $LASTEXITCODE 2

  $initDir = Join-Path $scratch "generated"
  New-Item -ItemType Directory -Path $initDir -Force | Out-Null
  $out = (& $Vpm run -- i --dir $initDir 2>&1 | Out-String)
  Check-Contains "init.message" $out "created"
  Check-Code "init.code" $LASTEXITCODE 0
  $created = Join-Path $initDir "hello.txt"
  if (Test-Path $created) {
    Write-Host "PASS init.file"
    $content = Get-Content -Raw $created
    Check-Contains "init.content" $content "Hello from hello-cli"
  } else {
    Write-Host "FAIL init.file (missing $created)"
    $script:failed = $script:failed + 1
  }
} finally {
  Pop-Location
  Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue
}

if ($failed -ne 0) {
  Write-Error ($failed.ToString() + " example check(s) failed")
  exit 1
}
Write-Host "hello-cli example: all checks passed"
