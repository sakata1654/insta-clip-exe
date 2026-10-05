# Insta Clip Studio
Download (yt-dlp) -> Edit (rounded frame, blurred background, auto shade) -> Subtitles -> Export.

## Get the .exe (pick one)
**A. No tools needed - GitHub builds it for you**
1. Create a GitHub repo and upload this whole folder (keep the `.github` folder).
2. Open the repo's **Actions** tab -> "Build Windows installer" -> **Run workflow**.
3. When it finishes (~10-15 min), download **Insta-Clip-Studio-Setup** from the run page. Unzip, run the .exe.

**B. Build on your own PC**
Double-click `Build-Windows.bat`. It installs Node, Rust and C++ build tools (via winget), bundles yt-dlp + ffmpeg, and builds the installer.

The installer bundles yt-dlp and ffmpeg and installs WebView2 if missing. Use "Update yt-dlp" in the Download tab if a site stops working.
