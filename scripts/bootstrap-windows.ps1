Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RootDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$AppDir = Join-Path $RootDir "app\src-tauri"
$ResDir = Join-Path $AppDir "resources"

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

function Check-MsvcLinker {
  Write-Log "Checking for MSVC compiler (cl.exe)..."
  # Try to find cl.exe using vswhere
  $vsPath = & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
  if ($vsPath) {
    Write-Log "Found Visual Studio at: $vsPath"
    return $true
  }
  return $false
}

Ensure-Command "winget"

$msvcInstalled = Check-MsvcLinker
if (-not $msvcInstalled) {
  Write-Log "MSVC not detected. Attempting to install..."
  Install-WingetPackage "Microsoft.VisualStudio.2022.BuildTools" @(
    "--silent",
    "--override",
    "--wait --passive --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
  )
} else {
  Write-Log "MSVC already installed. Skipping Build Tools install."
}
Install-WingetPackage "Microsoft.EdgeWebView2Runtime" @("--silent")
Install-WingetPackage "Rustlang.Rustup" @("--silent")
Install-WingetPackage "OpenJS.NodeJS.LTS" @("--silent")
Install-WingetPackage "yt-dlp.yt-dlp" @("--silent")
Install-WingetPackage "Gyan.FFmpeg" @("--silent")

Ensure-RustCargoPath

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  throw "Cargo not found after Rust install. Open a new terminal and run this script again."
}

Write-Log "Installing Tauri CLI"
cargo install tauri-cli --version "^2"

New-Item -ItemType Directory -Force -Path $ResDir | Out-Null
Write-Log "Installing latest yt-dlp sidecar"
Invoke-WebRequest -Uri "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe" -OutFile (Join-Path $ResDir "yt-dlp-win.exe")

Write-Log "Verifying backend build"
Push-Location $AppDir
# Check if link.exe is accessible (via Rust's discovery or PATH)
# We run a tiny cargo check to see if the environment is actually ready.
cargo check
$buildStatus = $LASTEXITCODE
Pop-Location

if ($buildStatus -ne 0) {
  Write-Host "" -ForegroundColor Yellow
  Write-Host "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!" -ForegroundColor Red
  Write-Host "BUILD FAILED: Linker 'link.exe' not found or failed." -ForegroundColor Red
  Write-Host "This is expected if Visual Studio Build Tools were just installed." -ForegroundColor Yellow
  Write-Host "1. Close ALL terminal windows." -ForegroundColor White
  Write-Host "2. (Optional but recommended) Restart your computer." -ForegroundColor White
  Write-Host "3. Open a NEW terminal and run this script again." -ForegroundColor White
  Write-Host "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!" -ForegroundColor Red
  exit 1
}

Write-Log "Done. Load extension from: $RootDir\extension"
Write-Log "Run desktop app: cd `"$AppDir`" ; cargo run"
