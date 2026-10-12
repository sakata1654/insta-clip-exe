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
