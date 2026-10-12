#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Mutex, OnceLock};
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{AppHandle, Emitter};

// ---------------------------------------------------------------- paths & tool discovery
static OVR: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn ovr(k: &str) -> Option<String> {
    let g = OVR.get_or_init(|| Mutex::new(HashMap::new())).lock().ok()?;
    g.get(k).cloned().filter(|s| !s.trim().is_empty())
}

#[tauri::command]
fn set_overrides(map: HashMap<String, String>) {
    if let Ok(mut g) = OVR.get_or_init(|| Mutex::new(HashMap::new())).lock() { *g = map; }
}

fn local_app() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("InstaClipStudio")
}
fn tools_dir() -> PathBuf { local_app().join("tools") }
fn models_dir() -> PathBuf { local_app().join("models") }
fn exe_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())).unwrap_or_default()
}

fn nowin(c: &mut Command) {
    #[cfg(windows)]
    { use std::os::windows::process::CommandExt; c.creation_flags(0x0800_0000); }
    #[cfg(not(windows))]
    { let _ = c; }
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for d in std::env::split_paths(&path) {
        for ext in [".exe", ""] {
            let p = d.join(format!("{name}{ext}"));
            if p.is_file() { return Some(p); }
        }
    }
    None
}

fn python() -> Option<(String, Vec<String>)> {
    let mut c: Vec<(String, Vec<String>)> = Vec::new();
    if let Some(o) = ovr("python") { c.push((o, vec![])); }
    c.push(("python".into(), vec![]));
    c.push(("py".into(), vec!["-3".into()]));
    c.push(("python3".into(), vec![]));
    for (p, pre) in c {
        let mut cm = Command::new(&p);
        nowin(&mut cm);
        if let Ok(o) = cm.args(&pre).args(["-c", "import sys;print(sys.executable)"]).output() {
            if o.status.success() && !o.stdout.is_empty() { return Some((p, pre)); }
        }
    }
    None
}

fn pycmd() -> Option<Command> {
    let (p, pre) = python()?;
    let mut c = Command::new(p);
    nowin(&mut c);
    c.args(pre);
    Some(c)
}

fn py_has(m: &str) -> bool {
    pycmd()
        .and_then(|mut c| c.args(["-c", &format!("import importlib.util as u,sys;sys.exit(0 if u.find_spec('{m}') else 1)")]).status().ok())
        .map(|s| s.success())
        .unwrap_or(false)
}

fn imageio_ffmpeg() -> Option<PathBuf> {
    static C: OnceLock<Option<PathBuf>> = OnceLock::new();
    C.get_or_init(|| {
        let mut c = pycmd()?;
        let o = c.args(["-c", "import imageio_ffmpeg;print(imageio_ffmpeg.get_ffmpeg_exe())"]).output().ok()?;
        if !o.status.success() { return None; }
        let p = PathBuf::from(String::from_utf8_lossy(&o.stdout).trim().to_string());
        if p.is_file() { Some(p) } else { None }
    }).clone()
}

// custom path -> tools dir (downloaded by Setup) -> next to the app -> PATH -> (ffmpeg) the copy bundled with moviepy
fn find_tool(name: &str) -> Option<PathBuf> {
    if let Some(o) = ovr(name) { let p = PathBuf::from(o); if p.is_file() { return Some(p); } }
    for d in [tools_dir(), exe_dir()] {
        for n in [format!("{name}.exe"), name.to_string()] {
            let p = d.join(&n);
            if p.is_file() { return Some(p); }
        }
    }
    if let Some(p) = which(name) { return Some(p); }
    if name == "ffmpeg" { return imageio_ffmpeg(); }
    None
}

fn find_whisper_cli() -> Option<PathBuf> {
    for d in [tools_dir().join("whisper"), exe_dir().join("whisper")] {
        for n in ["whisper-cli.exe", "main.exe"] {
            let p = d.join(n);
            if p.is_file() { return Some(p); }
        }
    }
    which("whisper-cli")
}

fn ffmpeg() -> Result<Command, String> {
    let p = find_tool("ffmpeg").ok_or("FFmpeg not found. Open the Setup tab and press Install missing.")?;
    let mut c = Command::new(p);
    nowin(&mut c);
    Ok(c)
}

fn ytdlp_cmd() -> Result<Command, String> {
    if let Some(p) = find_tool("yt-dlp") { let mut c = Command::new(p); nowin(&mut c); return Ok(c); }
    if py_has("yt_dlp") { if let Some(mut c) = pycmd() { c.args(["-m", "yt_dlp"]); return Ok(c); } }
    Err("yt-dlp not found. Open the Setup tab and press Install missing.".into())
}

fn hf_hub() -> PathBuf {
    if let Some(p) = std::env::var_os("HUGGINGFACE_HUB_CACHE") { return PathBuf::from(p); }
    if let Some(p) = std::env::var_os("HF_HOME") { return PathBuf::from(p).join("hub"); }
    std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default().join(".cache").join("huggingface").join("hub")
}

fn fw_models() -> Vec<String> {
    let mut v = vec![];
    if let Ok(rd) = std::fs::read_dir(hf_hub()) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if let Some(s) = n.strip_prefix("models--Systran--faster-whisper-") { v.push(s.to_string()); }
            else if let Some(s) = n.strip_prefix("models--Systran--faster-distil-whisper-") { v.push(format!("distil-{s}")); }
        }
    }
    v
}

fn cpp_models() -> Vec<String> {
    let mut v = vec![];
    for d in [models_dir(), tools_dir().join("whisper"), exe_dir().join("whisper")] {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if let Some(s) = n.strip_prefix("ggml-").and_then(|s| s.strip_suffix(".bin")) { v.push(s.to_string()); }
            }
        }
    }
    v
}

fn demucs_weights() -> bool {
    let base = std::env::var_os("TORCH_HOME").map(PathBuf::from)
        .unwrap_or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default().join(".cache").join("torch"));
    std::fs::read_dir(base.join("hub").join("checkpoints")).map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().ends_with(".th"))).unwrap_or(false)
}

// ---------------------------------------------------------------- helpers
fn run_stream(app: &AppHandle, ev: &str, mut c: Command) -> Result<ExitStatus, String> {
    let mut ch = c.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    let (a2, e2) = (app.clone(), ev.to_string());
    let err = ch.stderr.take().unwrap();
    let t = std::thread::spawn(move || { for l in BufReader::new(err).lines().flatten() { let _ = a2.emit(&e2, l); } });
    for l in BufReader::new(ch.stdout.take().unwrap()).lines().flatten() { let _ = app.emit(ev, l); }
    let _ = t.join();
    ch.wait().map_err(|e| e.to_string())
}

fn dec(s: &str) -> String {
    let b = s.as_bytes();
    let (mut o, mut i) = (Vec::new(), 0);
    while i < b.len() {
        if b[i] == b'%' && i + 3 <= b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) { o.push(v); i += 3; continue; }
        }
        o.push(b[i]); i += 1;
    }
    String::from_utf8_lossy(&o).into_owned()
}

fn dl(app: &AppHandle, url: &str, dest: &Path) -> Result<(), String> {
    if let Some(d) = dest.parent() { let _ = std::fs::create_dir_all(d); }
    let tmp = dest.with_extension("part");
    let mut c = Command::new("curl.exe");
    nowin(&mut c);
    c.args(["-L", "--fail", "-s", "-S", "-o"]).arg(&tmp).arg(url);
    let _ = app.emit("setup-log", format!("Downloading {url}"));
    let st = run_stream(app, "setup-log", c)?;
    if !st.success() { let _ = std::fs::remove_file(&tmp); return Err(format!("download failed ({st})")); }
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())
}

fn unzip(app: &AppHandle, zip: &Path, dest: &Path) -> Result<(), String> {
    let mut c = Command::new("powershell");
    nowin(&mut c);
    c.args(["-NoProfile", "-Command", &format!("Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force", zip.display(), dest.display())]);
    if run_stream(app, "setup-log", c)?.success() { Ok(()) } else { Err("could not unzip".into()) }
}

fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() { if let Some(f) = find_file(&p, name) { return Some(f); } }
        else if p.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case(name)).unwrap_or(false) { return Some(p); }
    }
    None
}

fn ensure_cpp_model(app: &AppHandle, model: &str) -> Result<PathBuf, String> {
    if !model.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') { return Err("bad model name".into()); }
    let file = format!("ggml-{model}.bin");
    for d in [tools_dir().join("whisper"), exe_dir().join("whisper"), models_dir()] {
        let p = d.join(&file);
        if p.is_file() { return Ok(p); }
    }
    let total_mb: f64 = match model { "tiny" => 75.0, "base" => 142.0, "small" => 466.0, "medium" => 1530.0, "large-v3" => 3100.0, "large-v3-turbo" => 1620.0, "large-v3-turbo-q5_0" => 574.0, _ => 500.0 };
    let dir = models_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(&file);
    let tmp = dir.join(format!("{file}.part"));
    let mut c = Command::new("curl.exe");
    nowin(&mut c);
    let mut child = c.args(["-L", "--fail", "-s", "-o"]).arg(&tmp)
        .arg(format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{file}"))
        .spawn().map_err(|e| format!("curl not available: {e}"))?;
    loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            if !st.success() { let _ = std::fs::remove_file(&tmp); return Err(format!("model download failed ({st})")); }
            break;
        }
        let mb = std::fs::metadata(&tmp).map(|m| m.len() as f64 / 1048576.0).unwrap_or(0.0);
        let _ = app.emit("model-log", format!("{}", ((mb / total_mb) * 100.0).min(99.0) as u32));
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}

// ---------------------------------------------------------------- environment check + installer
#[tauri::command]
async fn env_check() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let py = python();
        let mut pyver = Value::Null;
        let mut pkgs = json!({});
        if let Some((p, pre)) = &py {
            let mut c = Command::new(p);
            nowin(&mut c);
            if let Ok(o) = c.args(pre).arg("--version").output() {
                pyver = json!(format!("{}{}", String::from_utf8_lossy(&o.stdout).trim(), String::from_utf8_lossy(&o.stderr).trim()));
            }
            let script = std::env::temp_dir().join("ics_env.py");
            if std::fs::write(&script, include_str!("../env_check.py")).is_ok() {
                if let Some(mut c) = pycmd() {
                    if let Ok(o) = c.arg(&script).output() { pkgs = serde_json::from_slice::<Value>(&o.stdout).unwrap_or(json!({})); }
                }
            }
        }
        let yt = find_tool("yt-dlp");
        let ff = find_tool("ffmpeg");
        let ffsrc = ff.as_ref().map(|p| {
            let s = p.display().to_string();
            if s.contains("imageio_ffmpeg") { "found with moviepy" } else if s.starts_with(&tools_dir().display().to_string()) { "installed by this app" } else { "system" }
        });
        let wc = find_whisper_cli();
        Ok(json!({
            "python": { "ok": py.is_some(), "version": pyver },
            "pkgs": pkgs,
            "yt_dlp": { "ok": yt.is_some() || pkgs["yt_dlp"] == true, "path": yt.map(|p| p.display().to_string()), "module": pkgs["yt_dlp"] == true },
            "ffmpeg": { "ok": ff.is_some(), "path": ff.map(|p| p.display().to_string()), "source": ffsrc },
            "whispercpp": { "ok": wc.is_some(), "path": wc.map(|p| p.display().to_string()) },
            "fw_models": fw_models(),
            "cpp_models": cpp_models(),
            "demucs_weights": demucs_weights(),
            "tools_dir": tools_dir().display().to_string(),
        }).to_string())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn install_tool(app: AppHandle, name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let td = tools_dir();
        let log = |m: &str| { let _ = app.emit("setup-log", m.to_string()); };
        match name.as_str() {
            "ytdlp" => { dl(&app, "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe", &td.join("yt-dlp.exe"))?; }
            "ffmpeg" => {
                let zip = td.join("ffmpeg_dl.zip");
                let x = td.join("ffmpeg_x");
                dl(&app, "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip", &zip)?;
                log("Unpacking FFmpeg…");
                unzip(&app, &zip, &x)?;
                for n in ["ffmpeg.exe", "ffprobe.exe"] {
                    if let Some(f) = find_file(&x, n) { std::fs::copy(&f, td.join(n)).map_err(|e| e.to_string())?; }
                }
                let _ = std::fs::remove_dir_all(&x);
                let _ = std::fs::remove_file(&zip);
            }
            "whispercpp" => {
                let zip = td.join("whisper_dl.zip");
                let x = td.join("whisper_x");
                dl(&app, "https://github.com/ggml-org/whisper.cpp/releases/latest/download/whisper-bin-x64.zip", &zip)?;
                unzip(&app, &zip, &x)?;
                let exe = find_file(&x, "whisper-cli.exe").ok_or("whisper-cli.exe not found in the download")?;
                let dst = td.join("whisper");
                std::fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
                for e in std::fs::read_dir(exe.parent().unwrap()).map_err(|e| e.to_string())?.flatten() {
                    if e.path().is_file() { let _ = std::fs::copy(e.path(), dst.join(e.file_name())); }
                }
                let _ = std::fs::remove_dir_all(&x);
                let _ = std::fs::remove_file(&zip);
            }
            "faster_whisper" | "demucs" => {
                let pkg = if name == "demucs" { "demucs" } else { "faster-whisper" };
                let mut c = pycmd().ok_or("Python not found")?;
                c.args(["-m", "pip", "install", "--upgrade", "--progress-bar", "off", pkg]);
                if !run_stream(&app, "setup-log", c)?.success() { return Err(format!("pip install {pkg} failed")); }
            }
            "python" => {
                let mut c = Command::new("winget");
                nowin(&mut c);
                c.args(["install", "-e", "--id", "Python.Python.3.12", "--accept-package-agreements", "--accept-source-agreements"]);
                if !run_stream(&app, "setup-log", c)?.success() { return Err("winget could not install Python".into()); }
                log("Python installed — restart the app so it is picked up.");
            }
            n if n.starts_with("fwmodel:") => {
                let m = &n[8..];
                if !m.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') { return Err("bad model name".into()); }
                let mut c = pycmd().ok_or("Python not found")?;
                c.args(["-c", &format!("from faster_whisper import download_model; print(download_model('{m}'))")]);
                if !run_stream(&app, "setup-log", c)?.success() { return Err("model download failed".into()); }
            }
            n if n.starts_with("cppmodel:") => { ensure_cpp_model(&app, &n[9..])?; }
            _ => return Err("unknown item".into()),
        }
        Ok("ok".to_string())
    }).await.map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- files
#[tauri::command]
fn read_file(path: String) -> Result<Response, String> {
    std::fs::read(&path).map(Response::new).map_err(|e| format!("{path}: {e}"))
}

#[tauri::command]
fn write_file(request: Request<'_>) -> Result<(), String> {
    let InvokeBody::Raw(data) = request.body() else { return Err("no data".into()) };
    let p = request.headers().get("path").and_then(|h| h.to_str().ok()).map(dec).ok_or("no path header")?;
    if let Some(d) = Path::new(&p).parent() { let _ = std::fs::create_dir_all(d); }
    std::fs::write(&p, data).map_err(|e| format!("{p}: {e}"))
}

#[tauri::command]
fn reveal(path: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        let mut c = Command::new("explorer");
        if Path::new(&path).is_file() { c.arg(format!("/select,{}", path)); } else { c.arg(&path); }
        c.spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    { let _ = path; }
    Ok(())
}

// ---------------------------------------------------------------- yt-dlp
#[tauri::command]
fn ytdlp_version() -> Result<String, String> {
    let o = ytdlp_cmd()?.arg("--version").output().map_err(|e| format!("yt-dlp: {e}"))?;
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

#[tauri::command]
fn ytdlp_update() -> Result<String, String> {
    if find_tool("yt-dlp").is_some() {
        let o = ytdlp_cmd()?.arg("-U").output().map_err(|e| e.to_string())?;
        return Ok(format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string());
    }
    let mut c = pycmd().ok_or("Python not found")?;
    let o = c.args(["-m", "pip", "install", "-U", "yt-dlp"]).output().map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&o.stdout).lines().last().unwrap_or("").to_string())
}

#[tauri::command]
async fn ytdlp_download(app: AppHandle, url: String, fmt: String, dir: String, start: String, end: String) -> Result<String, String> {
    let (sel, extra): (&str, Vec<&str>) = match fmt.as_str() {
        "1080" => ("bv*[height<=1080]+ba/b[height<=1080]", vec!["--merge-output-format", "mp4"]),
        "720" => ("bv*[height<=720]+ba/b[height<=720]", vec!["--merge-output-format", "mp4"]),
        "audio" => ("ba/b", vec!["-x", "--audio-format", "mp3"]),
        _ => ("bv*+ba/b", vec!["--merge-output-format", "mp4"]),
    };
    let extra: Vec<String> = extra.into_iter().map(String::from).collect();
    let sel = sel.to_string();
    let san = |s: &str| s.trim().chars().filter(|c| c.is_ascii_alphanumeric() || *c == ':' || *c == '.').collect::<String>();
    let (s0, e0) = (san(&start), san(&end));
    tauri::async_runtime::spawn_blocking(move || {
        let mut c = ytdlp_cmd()?;
        let mut name = String::from("%(title).80s [%(id)s]");
        if !s0.is_empty() || !e0.is_empty() {
            let a = if s0.is_empty() { "0".to_string() } else { s0.clone() };
            let b = if e0.is_empty() { "inf".to_string() } else { e0.clone() };
            name.push_str(&format!(" {}-{}", a.replace(':', "."), b.replace(':', ".")));
            c.args(["--download-sections", &format!("*{a}-{b}"), "--force-keyframes-at-cuts"]);
        }
        name.push_str(".%(ext)s");
        c.args(["--newline", "--no-playlist", "--no-quiet", "--progress", "-f", &sel, "-P", &dir, "-o", &name, "--print", "after_move:filepath"]);
        if let Some(ff) = find_tool("ffmpeg") { c.arg("--ffmpeg-location").arg(&ff); }
        let mut child = c.args(&extra).arg(&url).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().map_err(|e| format!("could not start yt-dlp: {e}"))?;
        let err = child.stderr.take().unwrap();
        let a2 = app.clone();
        std::thread::spawn(move || { for l in BufReader::new(err).lines().flatten() { let _ = a2.emit("ytdlp-log", l); } });
        let mut last = String::new();
        for l in BufReader::new(child.stdout.take().unwrap()).lines().flatten() {
            if !l.starts_with('[') && !l.trim().is_empty() { last = l.clone(); }
            let _ = app.emit("ytdlp-log", l);
        }
        let st = child.wait().map_err(|e| e.to_string())?;
        if st.success() && !last.is_empty() { Ok(last) } else { Err(format!("yt-dlp exited with {st}")) }
    }).await.map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- transcription (your faster-whisper, or whisper.cpp)
#[tauri::command]
async fn transcribe(app: AppHandle, input: String, lang: String, maxlen: u32, translate: bool, model: String, prompt: String, ctx: bool, det: bool, engine: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let tmp = std::env::temp_dir();
        let wav = tmp.join("ics_audio.wav");
        let o = ffmpeg()?.args(["-y", "-i", &input, "-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le"]).arg(&wav)
            .output().map_err(|e| format!("ffmpeg: {e}"))?;
        if !o.status.success() {
            return Err(format!("ffmpeg failed: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("")));
        }
        let use_faster = match engine.as_str() { "faster" => true, "cpp" => false, _ => model != "large-v3-turbo-q5_0" && py_has("faster_whisper") };
        if use_faster {
            let script = tmp.join("ics_whisper.py");
            let argf = tmp.join("ics_whisper_args.json");
            let outj = tmp.join("ics_sub.json");
            let _ = std::fs::remove_file(&outj);
            std::fs::write(&script, include_str!("../whisper_run.py")).map_err(|e| e.to_string())?;
            std::fs::write(&argf, json!({"audio": wav.display().to_string(), "out": outj.display().to_string(), "model": model, "lang": lang, "prompt": prompt, "ctx": ctx, "det": det, "translate": translate}).to_string()).map_err(|e| e.to_string())?;
            let mut c = pycmd().ok_or("Python not found — choose the whisper.cpp engine or install Python in Setup")?;
            c.arg(&script).arg(&argf).env("PYTHONIOENCODING", "utf-8").env("PYTHONUTF8", "1");
            let st = run_stream(&app, "whisper-log", c)?;
            let _ = std::fs::remove_file(&wav);
            if !st.success() { return Err(format!("faster-whisper failed ({st}) — see Setup")); }
            let segs: Value = serde_json::from_str(&std::fs::read_to_string(&outj).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            return Ok(json!({ "segments": segs }).to_string());
        }
        // whisper.cpp
        let mp = ensure_cpp_model(&app, if model == "auto" { "small" } else { model.as_str() })?;
        let wexe = find_whisper_cli().ok_or("whisper.cpp not installed — open Setup and press Install missing")?;
        let prefix = tmp.join("ics_sub");
        let srt = tmp.join("ics_sub.srt");
        let _ = std::fs::remove_file(&srt);
        let mut c = Command::new(&wexe);
        nowin(&mut c);
        if let Some(d) = wexe.parent() { c.current_dir(d); }
        c.arg("-m").arg(&mp).arg("-f").arg(&wav).args(["-l", &lang, "-osrt", "-pp", "-of"]).arg(&prefix);
        if maxlen > 0 { c.args(["-ml", &maxlen.to_string(), "-sow"]); }
        if translate { c.arg("-tr"); }
        if !prompt.trim().is_empty() { c.arg("--prompt").arg(prompt.trim()); }
        if !ctx { c.args(["-mc", "0"]); }
        if det { c.args(["-tp", "0.0", "-nf"]); }
        let st = run_stream(&app, "whisper-log", c)?;
        let _ = std::fs::remove_file(&wav);
        if !st.success() { return Err(format!("whisper exited with {st}")); }
        let text = std::fs::read_to_string(&srt).map_err(|e| format!("no subtitle output: {e}"))?;
        Ok(json!({ "srt": text }).to_string())
    }).await.map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- final render: audio chain + MP4 encode
#[tauri::command]
async fn finalize_export(app: AppHandle, video: String, source: String, start: f64, dur: f64, speed: f64, enhance: bool, pre: String, vocals: bool, demucs: String, crf: u32, out: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let log = |m: &str| { let _ = app.emit("export-log", m.to_string()); };
        let work = PathBuf::from(&out).parent().map(|p| p.to_path_buf()).unwrap_or_else(std::env::temp_dir);
        let wav = work.join("audio_work.wav");
        let speed = if speed.is_finite() && speed >= 0.5 && speed <= 2.0 { speed } else { 1.0 };
        let mut src = if pre.trim().is_empty() { source.clone() } else { pre.clone() };
        let mut trimmed = false;
        if vocals && pre.trim().is_empty() {
            log("Extracting audio for vocal isolation…");
            let hq = work.join("audio_hq.wav");
            let o = ffmpeg()?.args(["-y", "-ss", &start.to_string(), "-t", &dur.to_string(), "-i", &source, "-vn", "-ac", "2", "-ar", "44100"]).arg(&hq).output().map_err(|e| format!("ffmpeg: {e}"))?;
            if o.status.success() {
                let sep = work.join("demucs_separated");
                let mut got: Option<PathBuf> = None;
                for dev in ["cuda", "cpu"] {
                    log(&format!("Demucs ({demucs}) on {dev}… this can take a while"));
                    if let Some(mut c) = pycmd() {
                        let r = c.args(["-m", "demucs.separate", "--two-stems", "vocals", "-n", &demucs, "-d", dev, "--out"]).arg(&sep).arg(&hq).stdout(Stdio::null()).stderr(Stdio::null()).status();
                        let v = sep.join(&demucs).join("audio_hq").join("vocals.wav");
                        if matches!(r, Ok(s) if s.success()) && v.is_file() { got = Some(v); break; }
                    }
                }
                match got { Some(v) => { src = v.display().to_string(); trimmed = true; } None => log("Demucs unavailable — using the original audio") }
            }
        }
        log("Processing audio…");
        let mut chain: Vec<String> = Vec::new();
        if (speed - 1.0).abs() > 0.001 { chain.push(format!("atempo={speed}")); }
        if enhance {
            if pre.trim().is_empty() {
                chain.push("adeclip,highpass=f=80,afftdn=nr=12:nf=-35,equalizer=f=250:width_type=q:width=1:g=-2,equalizer=f=3500:width_type=q:width=1:g=2.5,highshelf=f=9000:g=2".into());
            }
            chain.push("acompressor=threshold=0.125:ratio=3:attack=10:release=150:makeup=2,loudnorm=I=-16:TP=-1.5:LRA=11".into());
        }
        let af = chain.join(",");
        let mut c = ffmpeg()?;
        c.arg("-y");
        if !trimmed { c.args(["-ss", &start.to_string(), "-t", &dur.to_string()]); }
        c.arg("-i").arg(&src).args(["-vn", "-ac", "2", "-ar", "44100"]);
        if !af.is_empty() { c.args(["-af", &af]); }
        let o = c.arg(&wav).output().map_err(|e| format!("ffmpeg: {e}"))?;
        if !o.status.success() { return Err(format!("audio step failed: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or(""))); }
        for (enc, extra) in [("h264_nvenc", vec!["-preset", "p4", "-tune", "hq", "-rc", "vbr", "-cq", "19", "-b:v", "8500k"]), ("libx264", vec!["-preset", "medium"])] {
            log(&format!("Encoding video ({enc})…"));
            let q = crf.to_string();
            let mut c = ffmpeg()?;
            c.args(["-y", "-i", &video]).arg("-i").arg(&wav).args(["-map", "0:v:0", "-map", "1:a:0", "-vf", "fps=30", "-c:v", enc]);
            if enc == "libx264" { c.args(["-crf", &q]); }
            c.args(&extra).args(["-pix_fmt", "yuv420p", "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", "-c:a", "aac", "-b:a", "192k", "-af", "alimiter=limit=0.97", "-shortest", "-movflags", "+faststart"]).arg(&out);
            let o = c.output().map_err(|e| format!("ffmpeg: {e}"))?;
            if o.status.success() { let _ = std::fs::remove_file(&wav); return Ok(out.clone()); }
            if enc == "libx264" { return Err(format!("encode failed: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or(""))); }
        }
        Err("encode failed".into())
    }).await.map_err(|e| e.to_string())?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .invoke_handler(tauri::generate_handler![
            set_overrides, env_check, install_tool, read_file, write_file, reveal,
            ytdlp_version, ytdlp_update, ytdlp_download, transcribe, finalize_export
        ])
        .run(tauri::generate_context!())
        .expect("error while running app");
}
