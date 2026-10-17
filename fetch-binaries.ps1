# Build-time assets only (small). yt-dlp, FFmpeg, Whisper and Python tools are NOT bundled:
# the app detects what you already have on your PC and installs only what is missing (Setup tab).
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$root = Join-Path $PSScriptRoot '..'
try {
  $v = Join-Path $root 'src\vendor'; New-Item -ItemType Directory -Force "$v\wasm" | Out-Null
  $cdn = 'https://cdn.jsdelivr.net/npm/@mediapipe/tasks-vision@0.10.14'
  Invoke-WebRequest "$cdn/vision_bundle.mjs" -OutFile "$v\vision_bundle.mjs"
  foreach ($f in 'vision_wasm_internal.js','vision_wasm_internal.wasm','vision_wasm_nosimd_internal.js','vision_wasm_nosimd_internal.wasm') { Invoke-WebRequest "$cdn/wasm/$f" -OutFile "$v\wasm\$f" }
  Invoke-WebRequest 'https://storage.googleapis.com/mediapipe-models/face_detector/blaze_face_short_range/float16/1/blaze_face_short_range.tflite' -OutFile "$v\blaze_face_short_range.tflite"
} catch { Write-Warning "face detection files failed: $_" }
try {
  $f = Join-Path $root 'src\fonts'; New-Item -ItemType Directory -Force $f | Out-Null
  Invoke-WebRequest 'https://raw.githubusercontent.com/google/fonts/main/ofl/amiri/Amiri-Bold.ttf' -OutFile "$f\Amiri-Bold.ttf"
  Invoke-WebRequest 'https://raw.githubusercontent.com/google/fonts/main/ofl/amiriquran/AmiriQuran-Regular.ttf' -OutFile "$f\AmiriQuran-Regular.ttf"
  Invoke-WebRequest 'https://raw.githubusercontent.com/google/fonts/main/ofl/scheherazadenew/ScheherazadeNew-Regular.ttf' -OutFile "$f\ScheherazadeNew-Regular.ttf"
} catch { Write-Warning "font download failed: $_" }
Write-Host 'Build assets ready.'
