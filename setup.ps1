# One-shot: installs prerequisites, bundles yt-dlp + ffmpeg, builds the installer.
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
function Refresh { $env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User') + ";$env:USERPROFILE\.cargo\bin" }
function Has($c) { Refresh; [bool](Get-Command $c -ErrorAction SilentlyContinue) }
if (!(Get-Command winget -ErrorAction SilentlyContinue)) { throw 'winget is missing. Update "App Installer" from the Microsoft Store and run again.' }
if (!(Has 'node'))  { Write-Host '> Installing Node.js';  winget install -e --id OpenJS.NodeJS.LTS --accept-package-agreements --accept-source-agreements }
$vsw = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$hasVC = (Test-Path $vsw) -and (& $vsw -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
if (!$hasVC) { Write-Host '> Installing C++ Build Tools (large, be patient)'
  winget install -e --id Microsoft.VisualStudio.2022.BuildTools --accept-package-agreements --accept-source-agreements --override '--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended' }
if (!(Has 'rustc')) { Write-Host '> Installing Rust'; winget install -e --id Rustlang.Rustup --accept-package-agreements --accept-source-agreements }
Refresh
rustup default stable-msvc
Write-Host '> Fetching build assets'; & "$PSScriptRoot\fetch-binaries.ps1"
Write-Host '> Installing JS tooling'; npm install
Write-Host '> Generating icons';     npx tauri icon app-icon.png
Write-Host '> Building (first build takes 5-15 min)'; npm run tauri build
$out = Resolve-Path 'src-tauri\target\release\bundle\nsis'
Write-Host "`nDone. Installer is in: $out"; explorer $out
