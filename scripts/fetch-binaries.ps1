# Downloads yt-dlp and ffmpeg so they get bundled inside the installer.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$dir = Join-Path $PSScriptRoot '..\src-tauri\binaries'
New-Item -ItemType Directory -Force $dir | Out-Null
$t = 'x86_64-pc-windows-msvc'
if (!(Test-Path "$dir\yt-dlp-$t.exe")) {
  Invoke-WebRequest 'https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe' -OutFile "$dir\yt-dlp-$t.exe"
}
if (!(Test-Path "$dir\ffmpeg-$t.exe")) {
  $z = Join-Path $env:TEMP 'ffmpeg.zip'; $x = Join-Path $env:TEMP 'ffmpeg-x'
  Invoke-WebRequest 'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip' -OutFile $z
  Expand-Archive $z $x -Force
  Copy-Item (Get-ChildItem $x -Recurse -Filter ffmpeg.exe | Select-Object -First 1).FullName "$dir\ffmpeg-$t.exe"
}
Write-Host 'Bundled tools ready.'
