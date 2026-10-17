# Insta Clip Studio
Download (yt-dlp) -> Edit (studio card / simple crop, themes, camera, banner) -> Subtitles (faster-whisper, AI translate) -> Export (FFmpeg).

## Get the .exe (pick one)
**A. GitHub builds it for you**
1. Create a GitHub repo and upload this whole folder (keep the `.github` folder).
2. **Actions** tab -> "Build Windows installer" -> **Run workflow**.
3. Download **Insta-Clip-Studio-Setup** from the finished run, unzip, run the .exe.

**B. Build on your own PC:** double-click `Build-Windows.bat`.

## First launch = automatic setup
The installer is small. On first launch the **Setup** tab scans your PC and uses what you already have:
Python, faster-whisper (+ your cached large-v3 / distil-large-v3 / medium models), Demucs, yt-dlp, FFmpeg
(it even finds the FFmpeg that moviepy ships). Only what is missing is downloaded/installed
(yt-dlp, FFmpeg and whisper.cpp go to `%LOCALAPPDATA%\InstaClipStudio\tools`).
Custom locations can be set in the Setup tab.

Exports are saved to `Documents\Insta Clip Studio\Projects\Project_<date>`.
To update: re-upload changed files to the repo (same names overwrite) and run the workflow again.

## Models by language
- **Urdu**: Whisper large-v3 (faster-whisper). Cohere Transcribe has no Urdu.
- **Arabic**: Cohere Transcribe Arabic (July 2026) if set up, else Whisper.
- **English**: Cohere Transcribe if set up, else Whisper.
Cohere models are gated: add a free Hugging Face token in Setup and accept the terms on both model pages.
Cohere models give no timestamps, so subtitle timing comes from voice-activity detection.

## Translate tab
Gemini (or OpenAI / Claude / Groq / OpenRouter / Ollama). Lines are sent with their on-screen duration; results are cached.

## Editor tips
- **Timeline** (bottom): drag clips to move / resize, drag vertically to change lane, Ctrl+wheel to zoom. Keys: S split, M merge, D duplicate, Del delete.
- **Assets** tab: add images, videos, audio. Put two audio clips on different lanes to play them together. "Until the end" loops a clip or holds a picture.
- **Words** tab (or click a word on the preview): color / size for one word or every occurrence in the video.
- Extra audio clips are mixed by FFmpeg in the desktop export.

## v2 layout
Home (projects) -> four tabs: **Video editor** (project, download, assets, clip) · **Transcription** · **Translation** · **Style** (Video / Arabic / Translation targets).
Export is the button top right, Setup is the gear. Projects are saved automatically to `%LOCALAPPDATA%\InstaClipStudio\projects\<id>\` (media is referenced by its path, downloads go to the project's `media` folder).
New projects start as an empty 16:9 canvas; a video added to the timeline becomes a Video clip + a linked Audio clip. Right-click any clip: loop until the end, split, duplicate, unlink, delete.
