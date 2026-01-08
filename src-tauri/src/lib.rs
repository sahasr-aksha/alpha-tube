// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::sync::mpsc;
use std::thread;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::fs;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, State};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri_plugin_updater::UpdaterExt;

// Config for persistence
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AppConfig {
    pub download_dir: Option<String>,
}

impl AppConfig {
    pub fn load(app: &AppHandle) -> Self {
        let config_path = app.path().app_data_dir().unwrap().join("config.json");
        if config_path.exists() {
            let data = fs::read_to_string(config_path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or(AppConfig { download_dir: None })
        } else {
            AppConfig { download_dir: None }
        }
    }

    pub fn save(&self, app: &AppHandle) {
        let config_path = app.path().app_data_dir().unwrap().join("config.json");
        // Ensure dir exists
        let _ = fs::create_dir_all(config_path.parent().unwrap());
        let data = serde_json::to_string_pretty(self).unwrap();
        let _ = fs::write(config_path, data);
    }
}

#[derive(Clone, Serialize)]
pub struct DownloadProgress {
    pub id: String, // Unique ID for this download
    pub percent: f64,
    pub speed: String,
    pub eta: String,
    pub status: String, // "downloading", "processing", "complete", "error"
    pub filename: String,
}

#[derive(Clone, Serialize)]
pub struct VideoFormat {
    pub format_id: String,
    pub ext: String,
    pub resolution: String,
    pub fps: f64,
    pub filesize: u64,
    pub vcodec: String,
    pub acodec: String,
    pub note: String,
}

#[derive(Deserialize, Clone, Serialize, Debug)]
pub struct DownloadOptions {
    pub id: String,            // Unique ID for this download
    pub url: String,
    pub quality: String,       // e.g., "720p", "1080p", "best", "audio"
    pub output_path: String,
    pub format_id: Option<String>, // Optional specific format ID
}

#[derive(Clone, Serialize)]
pub struct DownloadState {
    pub options: DownloadOptions,
    pub child_pid: u32,
    pub paused: bool,
    pub current_filename: Option<String>,
}

pub type DownloadManager = Mutex<HashMap<String, DownloadState>>;

// New Commands for File Management

#[tauri::command]
async fn get_app_config(app: AppHandle) -> Result<AppConfig, String> {
    Ok(AppConfig::load(&app))
}

#[tauri::command]
async fn set_download_dir(app: AppHandle, path: String) -> Result<(), String> {
    let mut config = AppConfig::load(&app);
    config.download_dir = Some(path);
    config.save(&app);
    Ok(())
}

#[tauri::command]
async fn cancel_download(
    app: AppHandle,
    id: String,
    state: State<'_, DownloadManager>,
) -> Result<String, String> {
    let mut manager = state.lock().unwrap();
    
    if let Some(download_state) = manager.get(&id) {
        let pid = download_state.child_pid;
        let output_path = download_state.options.output_path.clone(); // Directory
        let filename = download_state.current_filename.clone();

        // Kill the process (Process Tree)
        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("taskkill")
                .creation_flags(0x08000000)
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .output();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = Command::new("kill")
                .arg(pid.to_string())
                .output();
        }
        
        // Remove from state immediately
        manager.remove(&id);

        // CLEANUP FILES
        if let Some(fname) = filename {
            let encoded_fname = fname; 
            // construct paths
            let base = std::path::Path::new(&output_path).join(&encoded_fname);
            let part = std::path::Path::new(&output_path).join(format!("{}.part", encoded_fname));
            let ytdl = std::path::Path::new(&output_path).join(format!("{}.ytdl", encoded_fname));
            
            // Try deleting all variants
            let _ = fs::remove_file(base);
            let _ = fs::remove_file(part);
            let _ = fs::remove_file(ytdl);
        }
        
        // Emit cancelled event (optional, or just error status)
        let _ = app.emit("download-progress", DownloadProgress {
            id: id.clone(),
            percent: 0.0,
            speed: String::new(),
            eta: String::new(),
            status: "cancelled".to_string(), // Frontend should handle this removal
            filename: String::new(),
        });
        
        Ok("Download cancelled".to_string())
    } else {
        Err("Download not active".to_string())
    }
}

#[tauri::command]
async fn delete_file(path: String) -> Result<String, String> {
    fs::remove_file(&path).map_err(|e| format!("Failed to delete file: {}", e))?;
    Ok("File deleted".to_string())
}

#[tauri::command]
async fn reveal_file_in_explorer(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .args(["/select,", &path]) // Comma is important for explorer select
            .spawn()
            .map_err(|e| format!("Failed to open explorer: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| format!("Failed to open finder: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        // Try xdg-open (might open parent dir) or specific file managers
        // Standard xdg-open usually just opens file if passed file path.
        // Opening parent dir is safer for "reveal"
        let parent = std::path::Path::new(&path).parent().unwrap_or(std::path::Path::new("/"));
        Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| format!("Failed to open file manager: {}", e))?;
    }

    Ok(())
}

#[derive(Clone, Serialize)]
pub struct VideoFile {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified_date: String,
    pub extension: String,
}

#[derive(Clone, Serialize)]
pub struct PlaylistFolder {
    pub name: String,
    pub path: String,
    pub video_count: usize,
    pub thumbnail_path: Option<String>,
    pub videos: Vec<VideoFile>,
}

#[derive(Clone, Serialize)]
pub struct LibraryContent {
    pub playlists: Vec<PlaylistFolder>,
    pub singles: Vec<VideoFile>,
}

#[derive(Clone, Serialize)]
pub struct VideoMetadataResponse {
    pub title: String,
    pub thumbnail_url: String,
    pub duration: f64,
    pub formats: Vec<VideoFormat>,
    pub is_playlist: bool,
    pub video_count: Option<usize>,
}

#[derive(Clone, Serialize)]
pub struct VideoMetadata {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub codec: String,
    pub file_size: u64,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Get the path to the sidecar binary
fn get_sidecar_path(app: &AppHandle, name: &str) -> Result<std::path::PathBuf, String> {
    // Try different binary name suffixes
    let suffixes = [
        "-x86_64-pc-windows-gnu.exe",
        "-x86_64-pc-windows-msvc.exe",
        ".exe", // Also try bare executable name
        "",     // And no extension (though unlikely on Windows for these tools)
    ];
    
    // Get possible base directories
    let mut search_paths: Vec<std::path::PathBuf> = Vec::new();
    
    // 1. Try resource_dir (works in release mode)
    if let Ok(resource_path) = app.path().resource_dir() {
        search_paths.push(resource_path.join("bin"));
    }
    
    // 2. Try relative to exe (works in dev mode - target/debug/)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            // In dev mode, exe is in target/debug/, but bins are in src-tauri/bin/
            // Go up to src-tauri/bin
            let dev_bin_path = exe_dir.parent()  // target/
                .and_then(|p| p.parent())         // src-tauri/
                .map(|p| p.join("bin"));
            if let Some(path) = dev_bin_path {
                search_paths.push(path);
            }
            
            // Also try alongside the exe
            search_paths.push(exe_dir.join("bin"));
        }
    }
    
    // 3. Try project root (where the user says they have downloaded ffmpeg)
    if let Ok(cwd) = std::env::current_dir() {
        search_paths.push(cwd.clone());
        search_paths.push(cwd.join("bin")); // specific bin folder in root
        search_paths.push(cwd.join("src-tauri").join("bin")); // standard tauri bin location
    }
    
    // Search for the binary
    for base_path in &search_paths {
        for suffix in &suffixes {
            let binary_name = format!("{}{}", name, suffix);
            let full_path = base_path.join(&binary_name);
            if full_path.exists() {
                return Ok(full_path);
            }
        }
    }
    
    Err(format!(
        "Sidecar '{}' not found. Searched in: {:?}",
        name,
        search_paths
    ))
}


/// Parse progress from yt-dlp output line
fn parse_progress(line: &str) -> Option<DownloadProgress> {
    // Match patterns like: [download]  45.2% of 10.5MiB at 1.2MiB/s ETA 00:05
    let download_regex = Regex::new(
        r"\[download\]\s+(\d+\.?\d*)%\s+of\s+~?\s*([\d.]+\w+)\s+at\s+([\d.]+\w+/s)(?:\s+ETA\s+(\S+))?"
    ).ok()?;
    
    if let Some(caps) = download_regex.captures(line) {
        let percent: f64 = caps.get(1)?.as_str().parse().ok()?;
        let speed = caps.get(3).map(|m| m.as_str().to_string()).unwrap_or_default();
        let eta = caps.get(4).map(|m| m.as_str().to_string()).unwrap_or_default();
        
        return Some(DownloadProgress {
            id: String::new(), // Placeholder, to be filled by caller
            percent,
            speed,
            eta,
            status: "downloading".to_string(),
            filename: String::new(),
        });
    }
    
    // Check for destination filename
    if line.contains("[download] Destination:") {
        let filename = line
            .split("Destination:")
            .nth(1)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        return Some(DownloadProgress {
            id: String::new(),
            percent: 0.0,
            speed: String::new(),
            eta: String::new(),
            status: "downloading".to_string(),
            filename,
        });
    }
    
    // Check for merging/processing
    if line.contains("[Merger]") || line.contains("[ExtractAudio]") || line.contains("[ffmpeg]") {
        return Some(DownloadProgress {
            id: String::new(),
            percent: 100.0,
            speed: String::new(),
            eta: String::new(),
            status: "processing".to_string(),
            filename: String::new(),
        });
    }
    
    // Check for already downloaded
    if line.contains("has already been downloaded") {
        return Some(DownloadProgress {
            id: String::new(),
            percent: 100.0,
            speed: String::new(),
            eta: String::new(),
            status: "complete".to_string(),
            filename: String::new(),
        });
    }
    
    None
}

/// Convert quality string to yt-dlp format selector
fn get_format_selector(quality: &str) -> String {
    match quality.to_lowercase().as_str() {
        "audio" | "mp3" | "audio only" => "bestaudio/best".to_string(),
        "360p" => "bestvideo[height<=360][ext=mp4]+bestaudio[ext=m4a]/best[height<=360][ext=mp4]/best[height<=360]".to_string(),
        "480p" => "bestvideo[height<=480][ext=mp4]+bestaudio[ext=m4a]/best[height<=480][ext=mp4]/best[height<=480]".to_string(),
        "720p" => "bestvideo[height<=720][ext=mp4]+bestaudio[ext=m4a]/best[height<=720][ext=mp4]/best[height<=720]".to_string(),
        "1080p" => "bestvideo[height<=1080][ext=mp4]+bestaudio[ext=m4a]/best[height<=1080][ext=mp4]/best[height<=1080]".to_string(),
        "1440p" | "2k" => "bestvideo[height<=1440][ext=mp4]+bestaudio[ext=m4a]/best[height<=1440][ext=mp4]/best[height<=1440]".to_string(),
        "2160p" | "4k" => "bestvideo[height<=2160][ext=mp4]+bestaudio[ext=m4a]/best[height<=2160][ext=mp4]/best[height<=2160]".to_string(),
        "best" | _ => "bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best".to_string(),
    }
}

/// Fetch video metadata for a URL (Title, Thumbnail, Duration, Formats)
#[tauri::command]
async fn get_video_metadata(app: AppHandle, url: String) -> Result<VideoMetadataResponse, String> {
    let yt_dlp_path = get_sidecar_path(&app, "yt-dlp")?;

    // STEP 1: Probe for Playlist or Video
    println!("Fetching metadata for URL: {}", url);
    
    // Use --flat-playlist to get minimal info quickly, and -J to get structured JSON
    // This serves as the primary probe.
    let mut cmd = Command::new(&yt_dlp_path);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    cmd.arg(&url)
        .arg("--flat-playlist")
        .arg("-J") // -J returns a single JSON object
        .arg("--socket-timeout")
        .arg("10") // 10 seconds timeout
        .stdout(Stdio::piped());

    println!("Running yt-dlp command...");
    let struct_output = cmd.output()
        .map_err(|e| format!("Failed to run yt-dlp probe: {}", e))?;
    
    println!("yt-dlp command finished with status: {}", struct_output.status);

    if !struct_output.status.success() {
         let stderr = String::from_utf8_lossy(&struct_output.stderr);
         println!("yt-dlp probe failed: {}", stderr);
         return Err(format!("yt-dlp probe failed: {}", stderr));
    }

    let struct_json_str = String::from_utf8_lossy(&struct_output.stdout);
    let struct_json: serde_json::Value = serde_json::from_str(&struct_json_str)
        .map_err(|e| format!("Failed to parse probe JSON: {}", e))?;

    let is_playlist = struct_json["_type"].as_str().unwrap_or("") == "playlist";

    if is_playlist {
        // It is a playlist! Return lightweight info
        let title = struct_json["title"].as_str().unwrap_or("Unknown Playlist").to_string();
        let count = struct_json["playlist_count"].as_u64()
             .or_else(|| struct_json["entries"].as_array().map(|a| a.len() as u64))
             .unwrap_or(0);
        
        // Try to get first video thumbnail if possible, or use standard
        let thumbnail_url = struct_json["thumbnails"].as_array()
            .and_then(|t| t.last()) // complex object
            .and_then(|t| t["url"].as_str())
            .map(|s| s.to_string())
            .unwrap_or_default();

        return Ok(VideoMetadataResponse {
            title,
            thumbnail_url,
            duration: 0.0,
            formats: Vec::new(), // No formats for list
            is_playlist: true,
            video_count: Some(count as usize),
        });
    }

    // STEP 2: Single Video Fallback (Existing Logic)
    // Use --no-playlist to ensure we only get the video
    println!("Fetching formats for single video...");
    let mut cmd2 = Command::new(&yt_dlp_path);
    #[cfg(target_os = "windows")]
    cmd2.creation_flags(0x08000000);

    cmd2.arg(&url)
        .arg("--dump-json")
        .arg("--no-playlist")
        .arg("--socket-timeout")
        .arg("10") // 10 seconds timeout
        .stdout(Stdio::piped());

    println!("Running yt-dlp format fetch...");
    let output = cmd2.output()
        .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;
    
    println!("yt-dlp format fetch finished with status: {}", output.status);
        
    if !output.status.success() {
        return Err(format!("yt-dlp failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    
    let json_str = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;
        
    let title = json["title"].as_str().unwrap_or("Unknown Title").to_string();
    let thumbnail_url = json["thumbnail"].as_str().unwrap_or("").to_string();
    let duration = json["duration"].as_f64().unwrap_or(0.0);
    
    let formats_array = json["formats"].as_array()
        .ok_or("No formats found")?;
        
    let mut video_formats: Vec<VideoFormat> = Vec::new();
    
    for fmt in formats_array {
        // Collect relevant fields
        let format_id = fmt["format_id"].as_str().unwrap_or("").to_string();
        let ext = fmt["ext"].as_str().unwrap_or("").to_string();
        let resolution = fmt["resolution"].as_str().map(|s| s.to_string()).unwrap_or_else(|| {
            // Try to construct resolution if missing
            if let (Some(w), Some(h)) = (fmt["width"].as_u64(), fmt["height"].as_u64()) {
                format!("{}x{}", w, h)
            } else {
                "audio only".to_string()
            }
        });
        let fps = fmt["fps"].as_f64().unwrap_or(0.0);
        let filesize = fmt["filesize"].as_u64()
            .or(fmt["filesize_approx"].as_u64())
            .unwrap_or(0);
        let vcodec = fmt["vcodec"].as_str().unwrap_or("none").to_string();
        let acodec = fmt["acodec"].as_str().unwrap_or("none").to_string();
        let note = fmt["format_note"].as_str().unwrap_or("").to_string();
        
        // Filter out obviously bad stuff or storyboards
        if format_id.contains("sb") || ext == "mhtml" {
            continue;
        }

        // We want:
        // 1. Video-only streams (vcodec != none, acodec == none) -> These usually have highest qualities
        // 2. Video+Audio streams (vcodec != none, acodec != none)
        // 3. Audio-only (if user wants to see them?) -> Let's keep them if requested, but for now focus on video options.
        // Actually, let's keep everything that looks like a video.
        
        if vcodec != "none" || (vcodec == "none" && acodec != "none") { // Include audio-only too
             video_formats.push(VideoFormat {
                format_id,
                ext,
                resolution,
                fps,
                filesize,
                vcodec,
                acodec,
                note,
            });
        }
    }
    
    Ok(VideoMetadataResponse {
        title,
        thumbnail_url,
        duration,
        formats: video_formats,
        is_playlist: false,
        video_count: None,
    })
}

/// Internal helper to run the download process
async fn download_video_internal(
    app: AppHandle,
    options: DownloadOptions,
    state: tauri::State<'_, DownloadManager>,
    is_resume: bool,
) -> Result<String, String> {
    let yt_dlp_path = get_sidecar_path(&app, "yt-dlp")?;
    let ffmpeg_path = get_sidecar_path(&app, "ffmpeg")?;
    
    let download_id = options.id.clone();
    
    let format_selector = if let Some(fid) = &options.format_id {
        // If a specific format ID is requested
        if options.quality.to_lowercase() == "mp3" {
            "bestaudio/best".to_string() 
        } else {
            // Force merge this video format with best audio
            format!("{}+bestaudio/best", fid)
        }
    } else {
        // Use quality preset selector
        get_format_selector(&options.quality)
    };
    
    // Build yt-dlp command
    let mut cmd_builder = Command::new(&yt_dlp_path);
    #[cfg(target_os = "windows")]
    cmd_builder.creation_flags(0x08000000);
    
    // Resolve Output Path (Config Preference)
    let final_output_path = if !is_resume {
        // Only override if new download, otherwise use stored options output_path
        // Actually, internal helper uses options.output_path directly. 
        // We should update options.output_path in the caller (download_video) if config exists.
        // For consistent logic, we just use options.output_path here.
        &options.output_path
    } else {
         &options.output_path
    };

    cmd_builder.arg(&options.url)
        .arg("-f")
        .arg(&format_selector)
        .arg("--ffmpeg-location")
        .arg(&ffmpeg_path)
        .arg("--merge-output-format")
        .arg("mp4")
        .arg("-o")
        // Output template: playlist videos go in subfolder, singles stay at root
        .arg(format!("{}/%(playlist_title,)s%(playlist_index|)s%(title)s.%(ext)s", final_output_path))
        .arg("--newline") // Each progress update on new line
        .arg("--no-colors") // Disable ANSI colors for easier parsing
        .arg("--concurrent-fragments")
        .arg("4")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    
    // Add audio extraction if audio quality selected
    if options.quality.to_lowercase() == "mp3" {
        cmd_builder.arg("-x")
        .arg("--audio-format")
        .arg("mp3");
    }

    // Spawn command
    let mut child = cmd_builder.spawn().map_err(|e| format!("Failed to spawn yt-dlp: {}", e))?;
    let child_pid = child.id();

    // REGISTER DOWNLOAD IN STATE
    {
        let mut manager = state.lock().unwrap();
        manager.insert(download_id.clone(), DownloadState {
            options: options.clone(),
            child_pid,
            paused: false,
            current_filename: None,
        });
    }
    
    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;
    
    let (tx, rx) = mpsc::channel::<String>();
    let tx_stderr = tx.clone();
    
    // Read stdout in a separate thread
    let stdout_thread = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                let _ = tx.send(line);
            }
        }
    });
    
    // Read stderr in a separate thread
    let stderr_thread = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                let _ = tx_stderr.send(line);
            }
        }
    });
    
    let mut last_filename = String::new();
    let mut error_output = String::new();
    
    // Process output lines and emit progress events
    while let Ok(line) = rx.recv() {
        // Check for errors
        if line.contains("ERROR:") {
            error_output = line.clone();
        }
        
        if let Some(mut progress) = parse_progress(&line) {
            // Inject ID
            progress.id = download_id.clone();

            // Preserve filename across progress updates
            if !progress.filename.is_empty() {
                last_filename = progress.filename.clone();
                // Update state with filename for cancellation cleanup
                let mut manager = state.lock().unwrap();
                if let Some(s) = manager.get_mut(&download_id) {
                    s.current_filename = Some(last_filename.clone());
                }
            } else if !last_filename.is_empty() {
                progress.filename = last_filename.clone();
            }
            
            let _ = app.emit("download-progress", progress);
        }
    }
    
    // Wait for threads to finish
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();
    
    // Wait for the process to finish
    let status = child.wait().map_err(|e| format!("Failed to wait for yt-dlp: {}", e))?;
    
    // CHECK IF PAUSED
    let is_paused = {
        let manager = state.lock().unwrap();
        if let Some(download_state) = manager.get(&download_id) {
            download_state.paused
        } else {
            false
        }
    };

    if is_paused {
        // Do not cleanup state, do not emit error/complete
        // The pause command has already updated the state to "paused"
        // and emitted the "paused" event if needed.
        
        // CRITICAL FIX: Emit "paused" event again here.
        // Reason: The `yt-dlp` process might have buffered output in the pipe (stdout/stderr)
        // containing "downloading" status updates. These are processed by the reading threads
        // *after* the process is killed but before this check.
        // This causes the frontend to flip back to "downloading" state after the initial "paused" event.
        // Emitting it here ensures the final state is correctly set to "paused".
        let _ = app.emit("download-progress", DownloadProgress {
            id: download_id,
            percent: 0.0, // Frontend preserves previous percent
            speed: String::new(),
            eta: String::new(),
            status: "paused".to_string(),
            filename: String::new(),
        });

        return Ok("Download paused".to_string());
    }

    // CLEANUP STATE (Only if not paused)
    {
        let mut manager = state.lock().unwrap();
        manager.remove(&download_id);
    }

    if status.success() {
        // CLEANUP: Delete any leftover .part and .ytdl files on successful completion
        if !last_filename.is_empty() {
            let part_file = format!("{}.part", last_filename);
            let ytdl_file = format!("{}.ytdl", last_filename);
            let _ = fs::remove_file(&part_file);
            let _ = fs::remove_file(&ytdl_file);
        }
        
        // Emit completion event
        let _ = app.emit("download-progress", DownloadProgress {
            id: download_id,
            percent: 100.0,
            speed: String::new(),
            eta: String::new(),
            status: "complete".to_string(),
            filename: last_filename.clone(),
        });
        Ok(format!("Download complete: {}", last_filename))
    } else {
        // Emit error event
        let _ = app.emit("download-progress", DownloadProgress {
            id: download_id,
            percent: 0.0,
            speed: String::new(),
            eta: String::new(),
            status: "error".to_string(),
            filename: error_output.clone(),
        });
        Err(format!("Download failed: {}", error_output))
    }
}

#[tauri::command]
async fn download_video(
    app: AppHandle,
    mut options: DownloadOptions, // Mutable to update path
    state: State<'_, DownloadManager>,
) -> Result<String, String> {
    // Check for configured download directory
    let config = AppConfig::load(&app);
    if let Some(dir) = config.download_dir {
        // Override output path if set
        options.output_path = dir;
    }
    
    download_video_internal(app, options, state, false).await
}

#[tauri::command]
async fn pause_download(
    app: AppHandle,
    id: String,
    state: State<'_, DownloadManager>,
) -> Result<String, String> {
    let mut manager = state.lock().unwrap();
    
    if let Some(download_state) = manager.get_mut(&id) {
        if download_state.paused {
            return Ok("Download already paused".to_string());
        }
        
        let pid = download_state.child_pid;
        
        // Kill the process
        #[cfg(target_os = "windows")]
        {
            let output = Command::new("taskkill")
                .creation_flags(0x08000000)
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .output();
                
            match output {
                Ok(o) => {
                    if !o.status.success() {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        println!("Failed to kill process {}: {}", pid, stderr);
                    } else {
                        println!("Successfully killed process {}", pid);
                    }
                },
                Err(e) => println!("Failed to execute taskkill: {}", e),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = Command::new("kill")
                .arg(pid.to_string())
                .output();
        }
        
        // Update state
        download_state.paused = true;
        
        // Emit paused event
        let _ = app.emit("download-progress", DownloadProgress {
            id: id.clone(),
            percent: 0.0, // Placeholder
            speed: String::new(),
            eta: String::new(),
            status: "paused".to_string(),
            filename: String::new(), // Frontend has filename
        });
        
        Ok("Download paused".to_string())
    } else {
        Err("Download not active".to_string())
    }
}

#[tauri::command]
async fn resume_download(
    app: AppHandle,
    id: String,
    state: State<'_, DownloadManager>,
) -> Result<String, String> {
    let options = {
        let manager = state.lock().unwrap();
        if let Some(download_state) = manager.get(&id) {
            if !download_state.paused {
                return Err("Download is not paused".to_string());
            }
            download_state.options.clone()
        } else {
            return Err("Download not found in history".to_string());
        }
    };
    
    // We do NOT remove the entry here because `download_video_internal` will update it (insert/overwrite).
    // Actually, `download_video_internal` inserts a new entry.
    // We should probably rely on `download_video_internal` to overwrite the existing entry with the new PID.
    
    // Emit resuming event
    let _ = app.emit("download-progress", DownloadProgress {
        id: id.clone(),
        percent: 0.0, 
        speed: "Resuming...".to_string(),
        eta: String::new(),
        status: "downloading".to_string(), // Switch back to downloading
        filename: String::new(),
    });

    download_video_internal(app, options, state, true).await
}

/// Store the YouTube WebView label for URL retrieval
static YOUTUBE_WEBVIEW_LABEL: &str = "youtube-browser";

/// Open YouTube in a new WebView window with persistent cookies
#[tauri::command]
async fn open_youtube_webview(app: AppHandle) -> Result<String, String> {
    // Check if WebView already exists
    if app.get_webview_window(YOUTUBE_WEBVIEW_LABEL).is_some() {
        // Focus the existing window
        if let Some(window) = app.get_webview_window(YOUTUBE_WEBVIEW_LABEL) {
            window.set_focus().map_err(|e| format!("Failed to focus window: {}", e))?;
        }
        return Ok("YouTube browser already open".to_string());
    }
    
    // Get app data directory for persistent cookies
    let data_dir = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .join("youtube_webview_data");
    
    // Create the WebView window with persistent data directory
    let _webview = WebviewWindowBuilder::new(
        &app,
        YOUTUBE_WEBVIEW_LABEL,
        WebviewUrl::External("https://www.youtube.com".parse().unwrap()),
    )
    .title("YouTube Browser - Alpha Tube")
    .inner_size(1000.0, 700.0)
    .center()
    .data_directory(data_dir)
    .build()
    .map_err(|e| format!("Failed to create WebView: {}", e))?;
    
    Ok("YouTube browser opened".to_string())
}




/// Get the current URL from the YouTube WebView
#[tauri::command]
async fn get_youtube_url(app: AppHandle) -> Result<String, String> {
    let webview = app.get_webview_window(YOUTUBE_WEBVIEW_LABEL)
        .ok_or("YouTube browser is not open")?;
    
    let url = webview.url()
        .map_err(|e| format!("Failed to get URL: {}", e))?;
    
    Ok(url.to_string())
}

/// Navigate the YouTube WebView to a specific URL
#[tauri::command]
async fn navigate_youtube(app: AppHandle, url: String) -> Result<String, String> {
    let webview = app.get_webview_window(YOUTUBE_WEBVIEW_LABEL)
        .ok_or("YouTube browser is not open")?;
    
    let parsed_url: tauri::Url = url.parse()
        .map_err(|e| format!("Invalid URL: {}", e))?;
    
    webview.navigate(parsed_url)
        .map_err(|e| format!("Failed to navigate: {}", e))?;
    
    Ok("Navigation successful".to_string())
}

/// Close the YouTube WebView window
#[tauri::command]
async fn close_youtube_webview(app: AppHandle) -> Result<String, String> {
    if let Some(webview) = app.get_webview_window(YOUTUBE_WEBVIEW_LABEL) {
        webview.close().map_err(|e| format!("Failed to close: {}", e))?;
        Ok("YouTube browser closed".to_string())
    } else {
        Ok("YouTube browser was not open".to_string())
    }
}

/// Scan a directory for video files
#[tauri::command]
async fn scan_downloads_directory(directory: String) -> Result<Vec<VideoFile>, String> {
    use std::fs;
    use std::time::UNIX_EPOCH;
    use chrono::{DateTime, Utc};
    
    let video_extensions = ["mp4", "mkv", "webm", "avi", "mov", "m4v", "flv", "wmv"];
    let mut video_files: Vec<VideoFile> = Vec::new();
    
    let entries = fs::read_dir(&directory)
        .map_err(|e| format!("Failed to read directory '{}': {}", directory, e))?;
    
    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    
                    if video_extensions.contains(&ext_str.as_str()) {
                        let metadata = fs::metadata(&path)
                            .map_err(|e| format!("Failed to get metadata: {}", e))?;
                        
                        let modified = metadata.modified()
                            .map(|time| {
                                let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
                                let datetime: DateTime<Utc> = DateTime::from_timestamp(duration.as_secs() as i64, 0)
                                    .unwrap_or_default();
                                datetime.format("%Y-%m-%d %H:%M").to_string()
                            })
                            .unwrap_or_else(|_| "Unknown".to_string());
                        
                        let name = path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        
                        video_files.push(VideoFile {
                            name,
                            path: path.to_string_lossy().to_string(),
                            size: metadata.len(),
                            modified_date: modified,
                            extension: ext_str,
                        });
                    }
                }
            }
        }
    }
    
    // Sort by modified date (newest first)
    video_files.sort_by(|a, b| b.modified_date.cmp(&a.modified_date));
    
    Ok(video_files)
}

/// Scan a directory for video files with playlist grouping
/// Subdirectories are treated as playlists, root files as singles
#[tauri::command]
async fn scan_library(directory: String) -> Result<LibraryContent, String> {
    use std::fs;
    use std::time::UNIX_EPOCH;
    use chrono::{DateTime, Utc};
    
    let video_extensions = ["mp4", "mkv", "webm", "avi", "mov", "m4v", "flv", "wmv"];
    let mut playlists: Vec<PlaylistFolder> = Vec::new();
    let mut singles: Vec<VideoFile> = Vec::new();
    
    let entries = fs::read_dir(&directory)
        .map_err(|e| format!("Failed to read directory '{}': {}", directory, e))?;
    
    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            
            if path.is_dir() {
                // This is a potential playlist folder
                let folder_name = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                
                // Skip hidden folders
                if folder_name.starts_with('.') {
                    continue;
                }
                
                let mut playlist_videos: Vec<VideoFile> = Vec::new();
                
                if let Ok(sub_entries) = fs::read_dir(&path) {
                    for sub_entry in sub_entries {
                        if let Ok(sub_entry) = sub_entry {
                            let sub_path = sub_entry.path();
                            
                            if sub_path.is_file() {
                                if let Some(ext) = sub_path.extension() {
                                    let ext_str = ext.to_string_lossy().to_lowercase();
                                    
                                    if video_extensions.contains(&ext_str.as_str()) {
                                        if let Ok(metadata) = fs::metadata(&sub_path) {
                                            let modified = metadata.modified()
                                                .map(|time| {
                                                    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
                                                    let datetime: DateTime<Utc> = DateTime::from_timestamp(duration.as_secs() as i64, 0)
                                                        .unwrap_or_default();
                                                    datetime.format("%Y-%m-%d %H:%M").to_string()
                                                })
                                                .unwrap_or_else(|_| "Unknown".to_string());
                                            
                                            let name = sub_path.file_name()
                                                .map(|n| n.to_string_lossy().to_string())
                                                .unwrap_or_default();
                                            
                                            playlist_videos.push(VideoFile {
                                                name,
                                                path: sub_path.to_string_lossy().to_string(),
                                                size: metadata.len(),
                                                modified_date: modified,
                                                extension: ext_str,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                // Only add folder as playlist if it contains videos
                if !playlist_videos.is_empty() {
                    // Sort videos in playlist by name (usually has index prefix)
                    playlist_videos.sort_by(|a, b| a.name.cmp(&b.name));
                    
                    // Use first video path as thumbnail hint
                    let thumbnail_path = playlist_videos.first().map(|v| v.path.clone());
                    
                    playlists.push(PlaylistFolder {
                        name: folder_name,
                        path: path.to_string_lossy().to_string(),
                        video_count: playlist_videos.len(),
                        thumbnail_path,
                        videos: playlist_videos,
                    });
                }
            } else if path.is_file() {
                // This is a single video file (not in a playlist folder)
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    
                    if video_extensions.contains(&ext_str.as_str()) {
                        if let Ok(metadata) = fs::metadata(&path) {
                            let modified = metadata.modified()
                                .map(|time| {
                                    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
                                    let datetime: DateTime<Utc> = DateTime::from_timestamp(duration.as_secs() as i64, 0)
                                        .unwrap_or_default();
                                    datetime.format("%Y-%m-%d %H:%M").to_string()
                                })
                                .unwrap_or_else(|_| "Unknown".to_string());
                            
                            let name = path.file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            
                            singles.push(VideoFile {
                                name,
                                path: path.to_string_lossy().to_string(),
                                size: metadata.len(),
                                modified_date: modified,
                                extension: ext_str,
                            });
                        }
                    }
                }
            }
        }
    }
    
    // Sort playlists by name, singles by modified date
    playlists.sort_by(|a, b| a.name.cmp(&b.name));
    singles.sort_by(|a, b| b.modified_date.cmp(&a.modified_date));
    
    Ok(LibraryContent { playlists, singles })
}

/// Get video metadata using ffprobe (LOCAL FILES)
#[tauri::command]
async fn get_local_video_metadata(app: AppHandle, video_path: String) -> Result<VideoMetadata, String> {
    let ffprobe_path = get_sidecar_path(&app, "ffprobe")?;
    
    let mut cmd = Command::new(&ffprobe_path);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    let output = cmd.args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            &video_path,
        ])
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;
    
    if !output.status.success() {
        return Err("ffprobe failed to analyze video".to_string());
    }
    
    let json_str = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse ffprobe output: {}", e))?;
    
    // Find video stream
    let streams = json["streams"].as_array()
        .ok_or("No streams found in video")?;
    
    let video_stream = streams.iter()
        .find(|s| s["codec_type"].as_str() == Some("video"))
        .ok_or("No video stream found")?;
    
    // Extract metadata
    let duration = json["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    
    let width = video_stream["width"].as_u64().unwrap_or(0) as u32;
    let height = video_stream["height"].as_u64().unwrap_or(0) as u32;
    let codec = video_stream["codec_name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    
    let file_size = json["format"]["size"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    
    Ok(VideoMetadata {
        duration,
        width,
        height,
        codec,
        file_size,
    })
}

/// Prepare HLS stream for local video if it has multiple audio tracks
#[tauri::command]
async fn prepare_hls_stream(app: AppHandle, video_path: String) -> Result<String, String> {
    // 1. Check for audio streams using ffprobe
    let ffprobe_path = get_sidecar_path(&app, "ffprobe")?;
    
    let mut probe_cmd = Command::new(&ffprobe_path);
    #[cfg(target_os = "windows")]
    probe_cmd.creation_flags(0x08000000);

    let probe_output = probe_cmd.args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_streams",
            "-select_streams", "a", // Select audio streams only
            &video_path,
        ])
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    if !probe_output.status.success() {
        return Err("ffprobe failed".to_string());
    }

    let probe_json: serde_json::Value = serde_json::from_slice(&probe_output.stdout)
        .map_err(|e| format!("Failed to parse probe output: {}", e))?;

    let audio_count = probe_json["streams"].as_array().map(|a| a.len()).unwrap_or(0);

    // If 1 or fewer audio streams, no need for HLS. Return original path.
    if audio_count <= 1 {
        // App expects asset url if not HLS? Or just path?
        // CyberPlayer uses convertFileSrc(videoPath). If we return path here, it works.
        println!("Video has {} audio streams. Using direct file playback.", audio_count);
        return Ok(video_path);
    }

    println!("Video has {} audio streams. Preparing HLS...", audio_count);

    // 2. Prepare HLS cache directory
    let data_dir = app.path().app_local_data_dir()
        .map_err(|e| format!("Failed to get app local data dir: {}", e))?
        .join("hls_cache");
    
    if !data_dir.exists() {
        fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create hls cache dir: {}", e))?;
    }

    // Hash video path to get unique folder name
    let mut hasher = DefaultHasher::new();
    video_path.hash(&mut hasher);
    let hash = hasher.finish();
    let stream_dir = data_dir.join(format!("{:x}", hash));

    if !stream_dir.exists() {
        fs::create_dir_all(&stream_dir).map_err(|e| format!("Failed to create stream dir: {}", e))?;
    }

    let master_playlist_path = stream_dir.join("master.m3u8");

    // 3. Check if HLS exists
    if master_playlist_path.exists() {
        println!("HLS cache found at {:?}", master_playlist_path);
        return Ok(master_playlist_path.to_string_lossy().to_string());
    }

    // 4. Transmux to HLS using ffmpeg
    // ffmpeg -i input.mp4 -map 0:v -map 0:a? -c:v copy -c:a copy -f hls 
    // -hls_time 10 -hls_list_size 0 -hls_segment_filename "seg_%03d.ts" master.m3u8
    
    let ffmpeg_path = get_sidecar_path(&app, "ffmpeg")?;
    
    // We use AC3/AAC copy? Vidstack likely needs AAC for best compatibility in HLS.
    // If codecs are weird, we might need to convert. But usually copy is fine for MP4 sources.
    // Let's stick to copy for speed.
    
    println!("Starting ffmpeg transmux...");
    let mut ffmpeg_cmd = Command::new(&ffmpeg_path);
    #[cfg(target_os = "windows")]
    ffmpeg_cmd.creation_flags(0x08000000);

    let status = ffmpeg_cmd.arg("-i")
        .arg(&video_path)
        .arg("-map")
        .arg("0:v") // Map all video streams (usually 1)
        .arg("-map")
        .arg("0:a?") // Map all audio streams (if any)
        .arg("-c:v")
        .arg("copy")
        .arg("-c:a")
        .arg("copy")
        .arg("-f")
        .arg("hls")
        .arg("-hls_time")
        .arg("4") // Reduced to 4s for better seeking/startup
        .arg("-hls_list_size")
        .arg("0")
        .arg("-hls_segment_type")
        .arg("fmp4") // Use Fragmented MP4 (better for copy mode)
        .arg("-hls_fmp4_init_filename")
        .arg(stream_dir.join("init.mp4")) // Fix: specify init file path explicitly
        .arg("-hls_segment_filename")
        .arg(stream_dir.join("seg_%03d.m4s"))
        .arg(&master_playlist_path)
        .status()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if status.success() {
        println!("HLS generation complete.");
        Ok(master_playlist_path.to_string_lossy().to_string())
    } else {
        Err("ffmpeg failed to generate HLS".to_string())
    }
}

/// YouTube Ad Blocker CSS - Cosmetic filters to hide ad elements
const YOUTUBE_ADBLOCK_SCRIPT: &str = r#"
(function() {
    'use strict';
    
    // Only run on YouTube
    if (!window.location.hostname.includes('youtube.com')) return;
    
    // CSS to hide YouTube ads
    const adBlockCSS = `
        /* Video player ads */
        .ytp-ad-module,
        .ytp-ad-overlay-container,
        .ytp-ad-text-overlay,
        .ytp-ad-player-overlay,
        .ytp-ad-image-overlay,
        .video-ads,
        .ytp-ad-progress-list,
        #player-ads,
        
        /* Homepage/sidebar ads */
        ytd-ad-slot-renderer,
        ytd-promoted-sparkles-web-renderer,
        ytd-display-ad-renderer,
        ytd-companion-slot-renderer,
        ytd-action-companion-ad-renderer,
        ytd-in-feed-ad-layout-renderer,
        ytd-banner-promo-renderer,
        ytd-statement-banner-renderer,
        ytd-brand-video-singleton-renderer,
        ytd-brand-video-shelf-renderer,
        
        /* Masthead/banner ads */
        #masthead-ad,
        .ytd-mealbar-promo-renderer,
        ytd-primetime-promo-renderer,
        
        /* Shorts ads */
        ytd-reel-video-renderer[is-ad],
        
        /* Search result ads */
        ytd-search-pyv-renderer,
        
        /* Movie/show promos */
        ytd-movie-offer-module-renderer,
        
        /* Premium promos */
        tp-yt-paper-dialog.ytd-popup-container,
        ytd-engagement-panel-section-list-renderer[target-id="engagement-panel-ads"],
        
        /* Overlay elements */
        .ytp-paid-content-overlay,
        .ytp-ad-overlay-slot {
            display: none !important;
        }
        
        /* Keep skip button visible if ads somehow still play */
        .ytp-ad-skip-button-container,
        .ytp-ad-skip-button {
            display: block !important;
            opacity: 1 !important;
        }
    `;
    
    // Inject CSS
    const style = document.createElement('style');
    style.textContent = adBlockCSS;
    document.head.appendChild(style);
    
    // Re-inject on navigation (YouTube is SPA)
    const observer = new MutationObserver(() => {
        if (!document.head.contains(style)) {
            document.head.appendChild(style);
        }
    });
    observer.observe(document.documentElement, { childList: true, subtree: true });
    
    // Auto-skip video ads if they manage to load
    setInterval(() => {
        // Click skip button if available
        const skipBtn = document.querySelector('.ytp-ad-skip-button, .ytp-skip-ad-button, .ytp-ad-skip-button-modern');
        if (skipBtn) skipBtn.click();
        
        // Skip overlay ads
        const overlayClose = document.querySelector('.ytp-ad-overlay-close-button');
        if (overlayClose) overlayClose.click();
    }, 500);
    
    console.log('[Alpha Tube] Ad blocker active');
})();
"#;







/// Check for updates silently and download if available
async fn check_and_download_update(app: AppHandle) {
    println!("[Updater] Checking for updates...");
    
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            println!("[Updater] Failed to get updater: {}", e);
            return;
        }
    };
    
    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => {
            println!("[Updater] No update available");
            return;
        }
        Err(e) => {
            println!("[Updater] Update check failed: {}", e);
            return;
        }
    };
    
    println!("[Updater] Update available: {} -> {}", update.current_version, update.version);
    
    // Download the update silently
    let downloaded = update.download(
        |_chunk_length, _content_length| {
            // Silent download - no progress reporting to user
        },
        || {
            println!("[Updater] Download complete, ready to install");
        }
    ).await;
    
    match downloaded {
        Ok(_) => {
            // Emit event to frontend that update is ready
            let _ = app.emit("update-ready", serde_json::json!({
                "version": update.version,
                "body": update.body.clone().unwrap_or_default()
            }));
            println!("[Updater] Emitted update-ready event");
        }
        Err(e) => {
            println!("[Updater] Download failed: {}", e);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Initialize Download Manager State
            app.manage(std::sync::Mutex::new(std::collections::HashMap::<String, DownloadState>::new()));

            #[cfg(target_os = "windows")]
            {
                use window_vibrancy::apply_acrylic;
                let window = app.get_webview_window("main").unwrap();
                // Apply Acrylic effect (blur behind window)
                let _ = apply_acrylic(&window, Some((0, 0, 0, 10)));
            }

            // Start background update checker
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Check on startup after a short delay
                tokio::time::sleep(Duration::from_secs(5)).await;
                check_and_download_update(app_handle.clone()).await;
                
                // Then check every 6 hours
                loop {
                    tokio::time::sleep(Duration::from_secs(6 * 60 * 60)).await;
                    check_and_download_update(app_handle.clone()).await;
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet, 
            download_video, 
            pause_download,
            resume_download,
            open_youtube_webview, 
            get_youtube_url, 
            navigate_youtube, 
            close_youtube_webview,
            scan_downloads_directory,
            scan_library,
            get_local_video_metadata,
            get_video_metadata,
            open_youtube_window,
            prepare_hls_stream,
            get_app_config, 
            set_download_dir, 
            cancel_download, 
            delete_file, 
            reveal_file_in_explorer
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Helper to create the YouTube window with AdBlock
fn create_youtube_window_internal(app: &AppHandle, url: &str) -> Result<(), String> {
    if app.get_webview_window("yt_browser").is_some() {
        let window = app.get_webview_window("yt_browser").unwrap();
        window.set_focus().map_err(|e| e.to_string())?;
        // Navigate if url is different? Or just focus.
        // If user wants to change URL, they should probably close and reopen, or use navigate_youtube command.
        // But for simplicity of "open with this URL", let's navigate if open.
         let parsed_url: tauri::Url = url.parse().map_err(|e| format!("Invalid URL: {}", e))?;
         window.navigate(parsed_url).map_err(|e| e.to_string())?;

        return Ok(());
    }

    // Get app data directory
    let data_dir = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .join("youtube_browser_data");

    // Only AdBlock script
    let combined_script = YOUTUBE_ADBLOCK_SCRIPT;

    WebviewWindowBuilder::new(
        app,
        "yt_browser",
        WebviewUrl::External(url.parse().unwrap()),
    )
    .title("YouTube Browser (AdBlock Active) - Alpha Tube")
    .inner_size(1100.0, 750.0)
    .center()
    .data_directory(data_dir)
    .initialization_script(combined_script)
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// Open YouTube in a new custom window with ad blocking
#[tauri::command]
async fn open_youtube_window(app: AppHandle, url: String) -> Result<(), String> {
    create_youtube_window_internal(&app, &url)
}





