Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RootDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$AppDir = Join-Path $RootDir "app\src-tauri"
$ResDir = Join-Path $AppDir "resources"

if ($env:Path -notlike "*C:\ProgramData\chocolatey\bin*") {
  $env:Path = "C:\ProgramData\chocolatey\bin;$env:Path"
}

function Write-Log($msg) {
  Write-Host "[hydrate-sidecars-windows] $msg"
}

function Ensure-Command($name) {
  $cmd = Get-Command $name -ErrorAction SilentlyContinue
  if (-not $cmd) {
    throw "Required command '$name' not found in PATH."
  }
  return $cmd
}

function Resolve-BundledBinarySource($name) {
  $cmd = Ensure-Command $name
  $path = $cmd.Source

  if ($path -like "C:\ProgramData\chocolatey\bin\*") {
    $root = if ($env:ChocolateyInstall) { $env:ChocolateyInstall } else { "C:\ProgramData\chocolatey" }
    $real = Get-ChildItem (Join-Path $root "lib") -Recurse -Filter "$name.exe" -ErrorAction SilentlyContinue |
      Where-Object { $_.FullName -notlike "*\chocolatey\bin\*" } |
      Sort-Object FullName |
      Select-Object -First 1
    if ($real) {
      return $real.FullName
    }
  }

  return $path
}

New-Item -ItemType Directory -Force -Path $ResDir | Out-Null

Write-Log "Downloading standalone yt-dlp Windows binary"
Invoke-WebRequest `
  -Uri "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe" `
  -OutFile (Join-Path $ResDir "yt-dlp-win.exe")

$ffmpegSource = Resolve-BundledBinarySource "ffmpeg"
$ffprobeSource = Resolve-BundledBinarySource "ffprobe"

Write-Log "Copying ffmpeg and ffprobe into bundled sidecars"
Copy-Item $ffmpegSource (Join-Path $ResDir "ffmpeg-win.exe") -Force
Copy-Item $ffprobeSource (Join-Path $ResDir "ffprobe-win.exe") -Force
