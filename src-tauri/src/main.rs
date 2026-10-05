#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tauri::{AppHandle, Emitter};

// Bundled yt-dlp / ffmpeg sit next to the app exe; fall back to PATH.
fn bin(name: &str) -> PathBuf {
    if let Some(d) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) {
        for n in [format!("{name}.exe"), name.to_string()] {
            let p = d.join(n);
            if p.is_file() { return p; }
        }
    }
    PathBuf::from(name)
}

fn cmd(name: &str) -> Command {
    let mut c = Command::new(bin(name));
    #[cfg(windows)]
    { use std::os::windows::process::CommandExt; c.creation_flags(0x0800_0000); } // no console window
    c
}

#[tauri::command]
fn ytdlp_version() -> Result<String, String> {
    let o = cmd("yt-dlp").arg("--version").output().map_err(|e| format!("yt-dlp not found: {e}"))?;
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

#[tauri::command]
fn ytdlp_update() -> Result<String, String> {
    let o = cmd("yt-dlp").arg("-U").output().map_err(|e| e.to_string())?;
    Ok(format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string())
}

#[tauri::command]
async fn ytdlp_download(app: AppHandle, url: String, fmt: String, dir: String) -> Result<String, String> {
    let (sel, extra): (&str, Vec<&str>) = match fmt.as_str() {
        "1080" => ("bv*[height<=1080]+ba/b[height<=1080]", vec!["--merge-output-format", "mp4"]),
        "720" => ("bv*[height<=720]+ba/b[height<=720]", vec!["--merge-output-format", "mp4"]),
        "audio" => ("ba/b", vec!["-x", "--audio-format", "mp3"]),
        _ => ("bv*+ba/b", vec!["--merge-output-format", "mp4"]),
    };
    let extra: Vec<String> = extra.into_iter().map(String::from).collect();
    let sel = sel.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let mut c = cmd("yt-dlp");
        c.args(["--newline", "--no-playlist", "--no-quiet", "--progress",
                "-f", &sel, "-P", &dir, "-o", "%(title).80s [%(id)s].%(ext)s",
                "--print", "after_move:filepath"]);
        let ff = bin("ffmpeg");
        if ff.is_file() { c.arg("--ffmpeg-location").arg(&ff); }
        let mut child = c.args(&extra).arg(&url)
            .stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().map_err(|e| format!("could not start yt-dlp: {e}"))?;
        let err = child.stderr.take().unwrap();
        let a2 = app.clone();
        std::thread::spawn(move || for l in BufReader::new(err).lines().flatten() { let _ = a2.emit("ytdlp-log", l); });
        let mut last = String::new();
        for l in BufReader::new(child.stdout.take().unwrap()).lines().flatten() {
            if !l.starts_with('[') && !l.trim().is_empty() { last = l.clone(); }
            let _ = app.emit("ytdlp-log", l);
        }
        let st = child.wait().map_err(|e| e.to_string())?;
        if st.success() && !last.is_empty() { Ok(last) } else { Err(format!("yt-dlp exited with {st}")) }
    }).await.map_err(|e| e.to_string())?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![ytdlp_version, ytdlp_update, ytdlp_download])
        .run(tauri::generate_context!())
        .expect("error while running app");
}
