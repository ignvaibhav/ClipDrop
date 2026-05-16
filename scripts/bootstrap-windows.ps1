Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RootDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$AppDir = Join-Path $RootDir "app\src-tauri"

function Write-Log($msg) {
  Write-Host "[bootstrap-windows] $msg"
}

function Ensure-Command($name) {
  if (-not (Get-Command $name -ErrorAction SilentlyContinue)) {
    throw "Required command '$name' not found in PATH."
  }
}

function Install-WingetPackage($id, $extraArgs = @()) {
  Write-Log "Installing $id"
  winget install -e --id $id --accept-package-agreements --accept-source-agreements @extraArgs
}

function Ensure-RustCargoPath {
  $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
  if ($env:Path -notlike "*$cargoBin*") {
    $env:Path = "$cargoBin;$env:Path"
  }
}

Ensure-Command "winget"

Install-WingetPackage "Microsoft.VisualStudio.2022.BuildTools" @(
  "--silent",
  "--override",
  "--wait --passive --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
)
Install-WingetPackage "Microsoft.EdgeWebView2Runtime" @("--silent")
Install-WingetPackage "Rustlang.Rustup" @("--silent")
Install-WingetPackage "OpenJS.NodeJS.LTS" @("--silent")
Install-WingetPackage "yt-dlp.yt-dlp" @("--silent")
Install-WingetPackage "Gyan.FFmpeg" @("--silent")

Ensure-RustCargoPath

if ($env:Path -notlike "*C:\ProgramData\chocolatey\bin*") {
  $env:Path = "C:\ProgramData\chocolatey\bin;$env:Path"
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  throw "Cargo not found after Rust install. Open a new terminal and run this script again."
}

Write-Log "Installing Tauri CLI"
cargo install tauri-cli --version "^2"

Write-Log "Hydrating bundled yt-dlp / ffmpeg / ffprobe sidecars"
& (Join-Path $RootDir "scripts\hydrate-sidecars-windows.ps1")

Write-Log "Verifying backend build"
Push-Location $AppDir
cargo check
Pop-Location

Write-Log "Done. Load extension from: $RootDir\extension"
Write-Log "Run desktop app: cd `"$AppDir`" ; cargo run"
