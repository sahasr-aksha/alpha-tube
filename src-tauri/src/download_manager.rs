#![allow(dead_code)]
#![allow(unused_variables)]

use std::process::{Command, Stdio};
use std::path::{Path, PathBuf};
use std::fs;
use tauri::{AppHandle, Manager, Emitter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::io::{BufRead, BufReader};
use std::thread;
use regex::Regex;
use crate::DownloadOptions;
use crate::DownloadProgress;

/// Identifies the type of stream being downloaded
#[derive(Debug, Clone, PartialEq)]
pub enum StreamType {
    Video,
    Audio,
    Combined,
}

#[derive(Debug)]
pub enum DownloadError {
    YtDlpError(String),
    Aria2Error(String),
    FFmpegError(String),
    IoError(std::io::Error),
    JsonError(serde_json::Error),
    Cancelled,
}

impl From<std::io::Error> for DownloadError {
    fn from(err: std::io::Error) -> Self {
        DownloadError::IoError(err)
    }
}

impl From<serde_json::Error> for DownloadError {
    fn from(err: serde_json::Error) -> Self {
        DownloadError::JsonError(err)
    }
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::YtDlpError(msg) => write!(f, "yt-dlp error: {}", msg),
            DownloadError::Aria2Error(msg) => write!(f, "aria2 error: {}", msg),
            DownloadError::FFmpegError(msg) => write!(f, "ffmpeg error: {}", msg),
            DownloadError::IoError(err) => write!(f, "IO error: {}", err),
            DownloadError::JsonError(err) => write!(f, "JSON error: {}", err),
            DownloadError::Cancelled => write!(f, "Download cancelled"),
        }
    }
}

impl DownloadError {
    /// Convert technical error to user-friendly message with actionable suggestions
    pub fn user_message(&self) -> String {
        match self {
            DownloadError::YtDlpError(msg) => Self::parse_ytdlp_error(msg),
            DownloadError::Aria2Error(msg) => Self::parse_aria2_error(msg),
            DownloadError::FFmpegError(msg) => Self::parse_ffmpeg_error(msg),
            DownloadError::IoError(err) => Self::parse_io_error(err),
            DownloadError::JsonError(_) => "Failed to parse video metadata. Try refreshing.".to_string(),
            DownloadError::Cancelled => "Download was cancelled.".to_string(),
        }
    }
    
    fn parse_ytdlp_error(msg: &str) -> String {
        let lower = msg.to_lowercase();
        
        if lower.contains("private video") || lower.contains("video is private") {
            return "This video is private and cannot be downloaded.".to_string();
        }
        if lower.contains("video unavailable") || lower.contains("not available") {
            return "This video is unavailable. It may have been removed or is restricted in your region.".to_string();
        }
        if lower.contains("age") || lower.contains("sign in") || lower.contains("login") {
            return "This video requires sign-in or age verification, which is not supported.".to_string();
        }
        if lower.contains("no video formats") || lower.contains("no suitable format") {
            return "No downloadable formats found for this video.".to_string();
        }
        if lower.contains("not a valid url") || lower.contains("unsupported url") {
            return "Invalid or unsupported URL. Please check the link.".to_string();
        }
        if lower.contains("connection") || lower.contains("network") || lower.contains("timed out") {
            return "Network error. Please check your internet connection and try again.".to_string();
        }
        if lower.contains("http error 429") || lower.contains("too many requests") {
            return "Rate limited by server. Please wait a few minutes and try again.".to_string();
        }
        if lower.contains("geo") || lower.contains("blocked") || lower.contains("country") {
            return "This video is blocked in your region.".to_string();
        }
        if lower.contains("copyright") {
            return "This video cannot be downloaded due to copyright restrictions.".to_string();
        }
        
        // Default fallback - truncate if too long
        if msg.len() > 80 {
            format!("Download failed: {}...", &msg[..80])
        } else {
            format!("Download failed: {}", msg)
        }
    }
    
    fn parse_aria2_error(msg: &str) -> String {
        let lower = msg.to_lowercase();
        
        if lower.contains("connection") || lower.contains("network") {
            return "Connection lost. Please check your internet and retry.".to_string();
        }
        if lower.contains("timeout") {
            return "Download timed out. The server may be slow - try again later.".to_string();
        }
        if lower.contains("disk") || lower.contains("space") {
            return "Not enough disk space. Free up some space and try again.".to_string();
        }
        if lower.contains("permission") || lower.contains("access denied") {
            return "Cannot write to download folder. Check folder permissions.".to_string();
        }
        
        "Download transfer failed. Please try again.".to_string()
    }
    
    fn parse_ffmpeg_error(msg: &str) -> String {
        let lower = msg.to_lowercase();
        
        if lower.contains("codec") || lower.contains("encoder") || lower.contains("decoder") {
            return "Media format not supported. Try a different quality.".to_string();
        }
        if lower.contains("permission") {
            return "Cannot create output file. Check folder permissions.".to_string();
        }
        if lower.contains("disk") || lower.contains("space") {
            return "Not enough disk space for merging. Free up space and retry.".to_string();
        }
        
        "Failed to merge video and audio. Try a different format.".to_string()
    }
    
    fn parse_io_error(err: &std::io::Error) -> String {
        match err.kind() {
            std::io::ErrorKind::PermissionDenied => "Permission denied. Check folder access rights.".to_string(),
            std::io::ErrorKind::NotFound => "File or folder not found. Check download location.".to_string(),
            std::io::ErrorKind::AlreadyExists => "File already exists.".to_string(),
            std::io::ErrorKind::OutOfMemory => "System out of memory. Close other applications.".to_string(),
            _ => format!("File system error: {}", err),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct YtDlpFormat {
    format_id: String,
    url: String,
    ext: String,
    vcodec: String,
    acodec: String,
    filesize: Option<u64>,
    http_headers: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct YtDlpJson {
    title: String,
    id: String,
    formats: Vec<YtDlpFormat>,
    requested_formats: Option<Vec<YtDlpFormat>>,
    url: Option<String>,
    ext: Option<String>,
    vcodec: Option<String>,
    acodec: Option<String>,
    http_headers: Option<serde_json::Value>,
    duration: Option<f64>,  // Video duration in seconds for progress calculation
}

/// Helper to get sidecar path (duplicated to avoid privacy issues with lib.rs for now, or we can make lib's public)
fn get_sidecar_path(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    // We'll reuse the logic from lib.rs or just call a shared helper if we refactor lib.rs
    // For this completely new file, it's safer to copy the logic to avoid "unused import" or visibility issues until integrated.
    let suffixes = [
        "-x86_64-pc-windows-gnu.exe",
        "-x86_64-pc-windows-msvc.exe",
        ".exe",
        "",
    ];
    
    let mut search_paths: Vec<PathBuf> = Vec::new();
    
    if let Ok(resource_path) = app.path().resource_dir() {
        search_paths.push(resource_path.join("bin"));
    }
    
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            search_paths.push(exe_dir.to_path_buf());
            search_paths.push(exe_dir.join("bin"));
            let dev_bin_path = exe_dir.parent().and_then(|p| p.parent()).map(|p| p.join("bin"));
            if let Some(path) = dev_bin_path {
                search_paths.push(path);
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        search_paths.push(cwd.clone());
        search_paths.push(cwd.join("bin"));
        search_paths.push(cwd.join("src-tauri").join("bin"));
    }
    
    for base_path in &search_paths {
        for suffix in &suffixes {
            let binary_name = format!("{}{}", name, suffix);
            let full_path = base_path.join(&binary_name);
            if full_path.exists() {
                return Ok(full_path);
            }
        }
    }
    
    Err(format!("Sidecar '{}' not found in {:?}", name, search_paths))
}

async fn fetch_metadata(app: &AppHandle, url: &str, quality_args: &[&str]) -> Result<(YtDlpJson, String), DownloadError> {
    let yt_dlp_path = get_sidecar_path(app, "yt-dlp").map_err(|e| DownloadError::YtDlpError(e))?;

    let mut cmd = Command::new(&yt_dlp_path);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    
    cmd.arg(url)
       .arg("--dump-json")
       .arg("--no-playlist");
       
    // Add quality args (format selector)
    for arg in quality_args {
        cmd.arg(arg);
    }

    let output = cmd.output().map_err(DownloadError::IoError)?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(DownloadError::YtDlpError(stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: YtDlpJson = serde_json::from_str(&stdout)?;
    
    // Also get the filename yt-dlp WOULD use
    let mut name_cmd = Command::new(&yt_dlp_path);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        name_cmd.creation_flags(0x08000000);
    }
    name_cmd.arg(url)
            .arg("--print")
            .arg("filename")
            .arg("-o")
            .arg("%(title)s.%(ext)s")
            .arg("--no-playlist");
            
    let name_output = name_cmd.output().map_err(DownloadError::IoError)?;
    let filename = String::from_utf8_lossy(&name_output.stdout).trim().to_string();

    Ok((json, filename))
}

/// Parsed progress data from aria2c output
#[derive(Debug, Clone)]
struct Aria2Progress {
    percent: f64,
    speed: String,
    eta: String,
}

/// Parse aria2c output line for progress info including speed and ETA
/// Example: [#2089b0 400.0KiB/34.0MiB(1%) CN:1 DL:115.0KiB ETA:4m59s]
/// Alternative: [#abc123 1.2GiB/2.4GiB(50%) CN:16 DL:25.3MiB/s ETA:0s]
fn parse_aria2_progress(line: &str) -> Option<Aria2Progress> {
    // Regex for percent: (XX.X%)
    let percent_re = Regex::new(r"\(([\d\.]+)%\)").ok()?;
    
    // Regex for download speed: DL:XXX.XKiB or DL:XXX.XMiB or DL:XXX.XGiB (may include /s)
    let speed_re = Regex::new(r"DL:([\d\.]+)\s*([KMG]i?B)(?:/s)?").ok()?;
    
    // Regex for ETA: ETA:Xm or ETA:XmYs or ETA:Xs or ETA:XhYmZs
    let eta_re = Regex::new(r"ETA:((?:\d+h)?(?:\d+m)?(?:\d+s)?)").ok()?;
    
    // Parse percent (required)
    let percent = percent_re.captures(line)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<f64>().ok())?;
    
    // Parse speed (optional, default to calculating later or showing "--")
    let speed = speed_re.captures(line)
        .map(|c| {
            let value = c.get(1).map(|m| m.as_str()).unwrap_or("0");
            let unit = c.get(2).map(|m| m.as_str()).unwrap_or("B");
            format!("{}{}/s", value, unit)
        })
        .unwrap_or_else(|| "-- B/s".to_string());
    
    // Parse ETA (optional)
    let eta = eta_re.captures(line)
        .and_then(|c| c.get(1))
        .map(|m| {
            let eta_str = m.as_str();
            if eta_str.is_empty() { "--:--".to_string() } else { eta_str.to_string() }
        })
        .unwrap_or_else(|| "--:--".to_string());
    
    Some(Aria2Progress { percent, speed, eta })
}

async fn download_stream(
    app: &AppHandle, 
    url: &str, 
    output_path: &Path, 
    headers: &serde_json::Value,
    download_id: String,
    should_cancel: Arc<AtomicBool>
) -> Result<(), DownloadError> {
    let aria2_path = get_sidecar_path(app, "aria2c").or_else(|_| get_sidecar_path(app, "aria2")).map_err(|e| DownloadError::Aria2Error(e))?;
    
    let mut cmd = Command::new(&aria2_path);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    
    cmd.arg(url)
       .arg("-o")
       .arg(output_path.file_name().unwrap())
       .arg("-d")
       .arg(output_path.parent().unwrap())
       .arg("-c") // Continue download
       .arg("--file-allocation=none")
       .arg("--summary-interval=1")
       .arg("--max-connection-per-server=8")
       .arg("--split=8");

    // Add headers
    if let Some(headers_map) = headers.as_object() {
        for (key, value) in headers_map {
            if let Some(v_str) = value.as_str() {
                cmd.arg(format!("--header={}: {}", key, v_str));
            }
        }
    }
    
    // Stdout/Stderr capture for progress
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    
    let mut child = cmd.spawn().map_err(DownloadError::IoError)?;
    let child_pid = child.id(); // For logging if needed, or cancellation implementation detail.
    
    // We need to continuously check `should_cancel` and read stdout
    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);
    
    let app_handle = app.clone();
    let d_id = download_id.clone();
    
    // We'll run a loop to monitor the process and cancellation
    // Since BufReader is blocking, we might need a separate thread or non-blocking approach.
    // simpler approach: Thread for reading lines -> sends to channel. Main loop checks channel and atomic bool.
    
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    
    thread::spawn(move || {
        for line in reader.lines() {
            if let Ok(l) = line {
                let _ = tx.send(l);
            } else {
                break;
            }
        }
    });

    loop {
        // Check cancellation
        if should_cancel.load(Ordering::Relaxed) {
             #[cfg(target_os = "windows")]
            {
                let _ = Command::new("taskkill")
                    .args(["/F", "/PID", &child_pid.to_string()])
                    .output();
            }
            #[cfg(not(target_os = "windows"))]
            {
                 let _ = Command::new("kill").arg(child_pid.to_string()).output();
            }
            return Err(DownloadError::Cancelled);
        }
        
        // Check output
        if let Ok(line) = rx.try_recv() {
            if let Some(progress) = parse_aria2_progress(&line) {
                 app_handle.emit("download-progress", DownloadProgress {
                    id: d_id.clone(),
                    percent: progress.percent,
                    speed: progress.speed,
                    eta: progress.eta,
                    status: "downloading".to_string(),
                    filename: output_path.file_name().unwrap().to_string_lossy().to_string(),
                    error_message: None,
                 }).unwrap_or(());
            }
        }
        
        // Check if finished
        if let Ok(Some(status)) = child.try_wait() {
            if status.success() {
                break;
            } else {
                return Err(DownloadError::Aria2Error("Process failed".to_string()));
            }
        }
        
        thread::sleep(std::time::Duration::from_millis(100));
    }

    Ok(())
}

/// Parse FFmpeg progress output for out_time_ms
/// Format: out_time_ms=123456789 (microseconds)
fn parse_ffmpeg_progress_time(line: &str) -> Option<f64> {
    if line.starts_with("out_time_ms=") {
        line.strip_prefix("out_time_ms=")
            .and_then(|v| v.trim().parse::<i64>().ok())
            .map(|us| us as f64 / 1_000_000.0) // Convert microseconds to seconds
    } else {
        None
    }
}

async fn merge_files(
    app: &AppHandle, 
    video_path: &Path, 
    audio_path: &Path, 
    output_path: &Path,
    should_cancel: Arc<AtomicBool>,
    download_id: &str,
    duration_secs: f64,
    output_filename: &str,
) -> Result<(), DownloadError> {
    let ffmpeg_path = get_sidecar_path(app, "ffmpeg").map_err(|e| DownloadError::FFmpegError(e))?;
    
    println!("[FFmpeg] Merging: {:?} + {:?} -> {:?} (duration: {:.1}s)", video_path, audio_path, output_path, duration_secs);
    
    // Emit initial muxing event at 0% IMMEDIATELY so UI shows "MUXING" status
    let _ = app.emit("download-progress", DownloadProgress {
        id: download_id.to_string(),
        percent: 0.0,
        speed: "Merging...".to_string(),
        eta: String::new(),
        status: "muxing".to_string(),
        filename: output_filename.to_string(),
        error_message: None,
    });
    
    let mut cmd = Command::new(&ffmpeg_path);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    
    cmd.arg("-i").arg(video_path)
       .arg("-i").arg(audio_path)
       .arg("-c").arg("copy")
       .arg("-y")
       .arg("-progress").arg("pipe:1")
       .arg("-nostats")
       .arg(output_path)
       .stdout(Stdio::piped())
       .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(DownloadError::IoError)?;
    let child_pid = child.id();
    
    // Take stdout for progress reading
    let stdout = child.stdout.take();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    
    // Spawn thread to read FFmpeg progress output
    if let Some(out) = stdout {
        thread::spawn(move || {
            let reader = BufReader::new(out);
            for line in reader.lines() {
                if let Ok(l) = line {
                    let _ = tx.send(l);
                }
            }
        });
    }
    
    let app_clone = app.clone();
    let id = download_id.to_string();
    let filename = output_filename.to_string();
    let mut last_percent = 0.0;
    
    loop {
        // Check cancellation
        if should_cancel.load(Ordering::Relaxed) {
            #[cfg(target_os = "windows")]
            {
                let _ = Command::new("taskkill").args(["/F", "/PID", &child_pid.to_string()]).output();
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = Command::new("kill").arg(child_pid.to_string()).output();
            }
            return Err(DownloadError::Cancelled);
        }
        
        // Process progress output
        while let Ok(line) = rx.try_recv() {
            // Debug log all FFmpeg progress lines
            if line.starts_with("out_time") || line.starts_with("progress") {
                println!("[FFmpeg Progress] {}", line);
            }
            
            if let Some(current_time) = parse_ffmpeg_progress_time(&line) {
                if duration_secs > 0.0 {
                    let percent = (current_time / duration_secs * 100.0).min(99.0);
                    // Only emit if significant change (reduces event spam)
                    if (percent - last_percent).abs() > 1.0 {
                        last_percent = percent;
                        let _ = app_clone.emit("download-progress", DownloadProgress {
                            id: id.clone(),
                            percent,
                            speed: "Merging...".to_string(),
                            eta: String::new(),
                            status: "muxing".to_string(),
                            filename: filename.clone(),
                            error_message: None,
                        });
                    }
                }
            }
        }
        
        // Check if FFmpeg finished
        if let Ok(Some(status)) = child.try_wait() {
            if status.success() {
                println!("[FFmpeg] Merge successful: {:?}", output_path);
                // Emit final 100% muxing complete
                let _ = app_clone.emit("download-progress", DownloadProgress {
                    id: id.clone(),
                    percent: 100.0,
                    speed: String::new(),
                    eta: String::new(),
                    status: "muxing".to_string(),
                    filename: filename.clone(),
                    error_message: None,
                });
                break;
            } else {
                // Capture stderr for debugging
                let stderr = child.stderr.take()
                    .map(|s| {
                        let reader = BufReader::new(s);
                        reader.lines().filter_map(|l| l.ok()).collect::<Vec<_>>().join("\n")
                    })
                    .unwrap_or_else(|| "Unknown error".to_string());
                println!("[FFmpeg] Merge failed: {}", stderr);
                return Err(DownloadError::FFmpegError(format!("Merge failed: {}", stderr)));
            }
        }
        thread::sleep(std::time::Duration::from_millis(50));
    }
    
    Ok(())
}

fn get_format_selector(quality: &str) -> String {
    // Re-implemented purely to be self-contained or import from lib if public
    match quality.to_lowercase().as_str() {
        "audio" | "mp3" => "bestaudio/best".to_string(),
        "4k" => "bestvideo[height<=2160]+bestaudio/best[height<=2160]".to_string(),
        "2k" => "bestvideo[height<=1440]+bestaudio/best[height<=1440]".to_string(),
        "1080p" => "bestvideo[height<=1080]+bestaudio/best[height<=1080]".to_string(),
        "720p" => "bestvideo[height<=720]+bestaudio/best[height<=720]".to_string(),
         _ => "bestvideo+bestaudio/best".to_string(),
    }
}

pub async fn download_pipeline(
    app: AppHandle,
    options: DownloadOptions,
    should_cancel: Arc<AtomicBool>,
) -> Result<String, DownloadError> {
    let download_id = options.id.clone();
    
    // 1. Fetch Metadata
    let quality_arg = get_format_selector(&options.quality);
    let (metadata, filename_template) = fetch_metadata(&app, &options.url, &["-f", &quality_arg]).await?;
    
    // Store duration for muxing progress calculation
    let video_duration = metadata.duration.unwrap_or(0.0);
    
    // Determine streams
    // If requested_formats exists (merged formats), use those.
    // If only 'formats' exists, it might be a single file or yt-dlp decided one.
    // But since we used -f, yt-dlp usually returns the 'requested_downloads' or similar in JSON if it's a split.
    // Actually, `requested_formats` tuple field in JSON holds the [video, audio] or [combined].
    
    let output_dir = Path::new(&options.output_path);
    if !output_dir.exists() {
        fs::create_dir_all(output_dir)?;
    }
    
    // Clean filename and ensure .mp4 extension for merged output
    let re = Regex::new(r#"[<>:"/\\|?*]"#).unwrap();
    let safe_filename = re.replace_all(&filename_template, "_").to_string();
    
    // For merged files, always output as .mp4 regardless of source format
    let final_filename = if metadata.requested_formats.is_some() {
        // Will need merge - force .mp4 extension
        let base = Path::new(&safe_filename)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or(safe_filename.clone());
        format!("{}.mp4", base)
    } else {
        safe_filename.clone()
    };
    let final_path = output_dir.join(&final_filename);
    
    // Tasks now include StreamType for proper identification
    let mut tasks: Vec<(String, PathBuf, serde_json::Value, StreamType)> = Vec::new();
    
    if let Some(req_fmts) = metadata.requested_formats {
        // Dual stream: separate video and audio
        for fmt in req_fmts.iter() {
            let url = fmt.url.clone();
            let headers = fmt.http_headers.clone();
            let ext = &fmt.ext;
            
            // Determine stream type from codecs
            let stream_type = if fmt.vcodec != "none" && (fmt.acodec == "none" || fmt.acodec.is_empty()) {
                StreamType::Video
            } else if (fmt.vcodec == "none" || fmt.vcodec.is_empty()) && fmt.acodec != "none" {
                StreamType::Audio
            } else {
                StreamType::Combined
            };
            
            // Create descriptive temp filename
            let temp_filename = match stream_type {
                StreamType::Video => format!("{}.video.{}", safe_filename, ext),
                StreamType::Audio => format!("{}.audio.{}", safe_filename, ext),
                StreamType::Combined => format!("{}.combined.{}", safe_filename, ext),
            };
            let temp_path = output_dir.join(&temp_filename);
            
            tasks.push((url, temp_path, headers, stream_type));
        }
    } else {
        // Single combined stream
        let url = metadata.url.ok_or(DownloadError::YtDlpError("No URL found".to_string()))?;
        let headers = metadata.http_headers.unwrap_or(serde_json::Value::Null);
        let ext = metadata.ext.unwrap_or("mp4".to_string());
        let temp_filename = format!("{}.combined.{}", safe_filename, ext);
        let temp_path = output_dir.join(&temp_filename);
        tasks.push((url, temp_path, headers, StreamType::Combined));
    }
    
    // Track downloaded files with their stream types
    let mut downloaded_files: Vec<(PathBuf, StreamType)> = Vec::new();
    
    // 2. Download Streams in parallel using tokio::join!
    let app_clone = app.clone();
    let cancel_clone = should_cancel.clone();
    let id_clone = download_id.clone();
    
    if tasks.len() == 2 {
        let t1 = tasks[0].clone();
        let t2 = tasks[1].clone();
        
        let h1 = download_stream(&app_clone, &t1.0, &t1.1, &t1.2, id_clone.clone(), cancel_clone.clone());
        let h2 = download_stream(&app_clone, &t2.0, &t2.1, &t2.2, id_clone.clone(), cancel_clone.clone());
        
        let (r1, r2) = tokio::join!(h1, h2);
        
        // If either fails, cleanup both temp files before returning error
        // Use match to consume and handle results properly
        match (r1, r2) {
            (Ok(_), Ok(_)) => {
                downloaded_files.push((t1.1, t1.3));
                downloaded_files.push((t2.1, t2.3));
            }
            (Err(e), _) => {
                let _ = fs::remove_file(&t1.1);
                let _ = fs::remove_file(&t2.1);
                return Err(e);
            }
            (_, Err(e)) => {
                let _ = fs::remove_file(&t1.1);
                let _ = fs::remove_file(&t2.1);
                return Err(e);
            }
        }
    } else if tasks.len() == 1 {
        let t1 = tasks[0].clone();
        if let Err(e) = download_stream(&app_clone, &t1.0, &t1.1, &t1.2, id_clone, cancel_clone.clone()).await {
            let _ = fs::remove_file(&t1.1);
            return Err(e);
        }
        downloaded_files.push((t1.1, t1.3));
    }
    
    // 3. Merge if needed
    if downloaded_files.len() == 2 {
        // Smart ordering: ensure video comes first, audio second for FFmpeg
        let (video_path, audio_path) = if downloaded_files[0].1 == StreamType::Video {
            (&downloaded_files[0].0, &downloaded_files[1].0)
        } else {
            (&downloaded_files[1].0, &downloaded_files[0].0)
        };
        
        // Output final
        if final_path.exists() {
            let _ = fs::remove_file(&final_path);
        }
        
        // Emit muxing status for UI feedback
        let _ = app.emit("download-progress", DownloadProgress {
            id: download_id.clone(),
            percent: 99.0,
            speed: String::new(),
            eta: String::new(),
            status: "muxing".to_string(),
            filename: final_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            error_message: None,
        });
        
        // Attempt merge with progress tracking
        let output_filename = final_path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let merge_result = merge_files(
            &app, 
            video_path, 
            audio_path, 
            &final_path, 
            should_cancel.clone(),
            &download_id,
            video_duration,
            &output_filename,
        ).await;
        
        // Only cleanup temp files on successful merge
        if merge_result.is_ok() {
            println!("[Download] Merge successful, cleaning up temp files");
            for (path, _) in &downloaded_files {
                let _ = fs::remove_file(path);
            }
        } else {
            println!("[Download] Merge failed, keeping temp files for debugging");
        }
        
        // Return error if merge failed
        merge_result?;
        
    } else if downloaded_files.len() == 1 {
        // Just rename combined/single stream
        if final_path.exists() {
            let _ = fs::remove_file(&final_path);
        }
        fs::rename(&downloaded_files[0].0, &final_path)?;
    }
    
    Ok(final_path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_aria2_full_progress() {
        let line = "[#2089b0 400.0KiB/34.0MiB(1%) CN:1 DL:115.0KiB ETA:4m59s]";
        let result = parse_aria2_progress(line);
        assert!(result.is_some());
        let p = result.unwrap();
        assert!((p.percent - 1.0).abs() < 0.1);
        assert!(p.speed.contains("KiB"));
        assert!(p.eta.contains("4m"));
    }

    #[test]
    fn test_parse_aria2_mib_speed() {
        let line = "[#abc123 1.2GiB/2.4GiB(50%) CN:16 DL:25.3MiB ETA:1m30s]";
        let result = parse_aria2_progress(line);
        assert!(result.is_some());
        let p = result.unwrap();
        assert!((p.percent - 50.0).abs() < 0.1);
        assert!(p.speed.contains("25.3"));
        assert!(p.speed.contains("MiB"));
        assert_eq!(p.eta, "1m30s");
    }

    #[test]
    fn test_parse_aria2_no_eta() {
        let line = "[#123456 500KiB/1MiB(50%) DL:100KiB]";
        let result = parse_aria2_progress(line);
        assert!(result.is_some());
        let p = result.unwrap();
        assert!((p.percent - 50.0).abs() < 0.1);
        assert!(p.speed.contains("100"));
    }

    #[test]
    fn test_parse_aria2_no_match() {
        let line = "Some random log message";
        let result = parse_aria2_progress(line);
        assert!(result.is_none());
    }

    #[test]
    fn test_user_message_private_video() {
        let err = DownloadError::YtDlpError("ERROR: Video is private".to_string());
        let msg = err.user_message();
        assert!(msg.contains("private"));
    }

    #[test]
    fn test_user_message_unavailable() {
        let err = DownloadError::YtDlpError("ERROR: Video unavailable".to_string());
        let msg = err.user_message();
        assert!(msg.contains("unavailable") || msg.contains("removed"));
    }

    #[test]
    fn test_user_message_network() {
        let err = DownloadError::YtDlpError("Connection refused".to_string());
        let msg = err.user_message();
        assert!(msg.contains("Network") || msg.contains("internet"));
    }

    #[test]
    fn test_user_message_cancelled() {
        let err = DownloadError::Cancelled;
        let msg = err.user_message();
        assert!(msg.contains("cancelled"));
    }

    #[test]
    fn test_user_message_ffmpeg_codec() {
        let err = DownloadError::FFmpegError("Decoder codec not found".to_string());
        let msg = err.user_message();
        assert!(msg.contains("format") || msg.contains("quality"));
    }
}
