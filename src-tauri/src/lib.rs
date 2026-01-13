#![allow(dead_code)]
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod search;
mod progress;
mod playlist;
mod download_manager;
mod stream_proxy;
mod url_cache;

use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::fs;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, State};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri_plugin_updater::UpdaterExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>, // User-friendly error message when status is "error"
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
    pub tbr: f64,  // Total bitrate in kbps
}

#[derive(Deserialize, Clone, Serialize, Debug)]
pub struct DownloadOptions {
    pub id: String,            // Unique ID for this download
    pub url: String,
    pub quality: String,       // e.g., "720p", "1080p", "best", "audio"
    pub output_path: String,
    pub format_id: Option<String>, // Optional specific format ID
}

#[derive(Clone)]
pub struct DownloadState {
    pub options: DownloadOptions,
    pub should_cancel: Arc<AtomicBool>,
    pub paused: bool,
    pub current_filename: Option<String>,
}

pub type DownloadManager = Mutex<HashMap<String, DownloadState>>;

// Playback state for HLS streaming processes
#[derive(Clone, Serialize)]
pub struct PlaybackState {
    pub child_pid: u32,
    pub video_path: String,
    pub stream_hash: String,
}

pub type PlaybackManager = Mutex<HashMap<String, PlaybackState>>;

/// Response for get_all_streaming_urls - contains URLs for all available qualities
#[derive(Clone, Serialize)]
pub struct StreamingUrls {
    pub urls: HashMap<String, String>,  // "480p" -> "https://..." (raw YouTube URLs for backend use)
    pub available: Vec<String>,          // ["360p", "480p", "720p", "1080p"]
    pub default_quality: String,         // "480p" - recommended default
    pub proxy_url: String,               // "http://127.0.0.1:9876/stream" - USE THIS FOR PLAYBACK
}

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
        // Signal cancellation
        download_state.should_cancel.store(true, Ordering::Relaxed);
        
        let output_path = download_state.options.output_path.clone(); // Directory
        let filename = download_state.current_filename.clone();

        // Remove from state immediately
        manager.remove(&id);

        // CLEANUP FILES
        if let Some(fname) = filename {
            // Simplified cleanup - download manager handles some, but we can double check
            let base = std::path::Path::new(&output_path).join(&fname);
            let part = std::path::Path::new(&output_path).join(format!("{}.part", fname));
            let ytdl = std::path::Path::new(&output_path).join(format!("{}.ytdl", fname));
            let _ = fs::remove_file(base);
            let _ = fs::remove_file(part);
            let _ = fs::remove_file(ytdl);
        }
        
        // Emit cancelled event
        let _ = app.emit("download-progress", DownloadProgress {
            id: id.clone(),
            percent: 0.0,
            speed: String::new(),
            eta: String::new(),
            status: "cancelled".to_string(),
            filename: String::new(),
            error_message: None,
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
    pub audio_tracks: Vec<String>,
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
    
    // 2. Try relative to exe (works in both dev and release mode)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            // PRIORITY: Try the executable directory directly (release/installed builds)
            // In installed apps, binaries are placed alongside the main exe
            search_paths.push(exe_dir.to_path_buf());
            
            // Also try bin subfolder alongside exe
            search_paths.push(exe_dir.join("bin"));
            
            // In dev mode, exe is in target/debug/, but bins are in src-tauri/bin/
            // Go up to src-tauri/bin
            let dev_bin_path = exe_dir.parent()  // target/
                .and_then(|p| p.parent())         // src-tauri/
                .map(|p| p.join("bin"));
            if let Some(path) = dev_bin_path {
                search_paths.push(path);
            }
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

/// Log download errors to a file for debugging
fn log_download_error(download_id: &str, phase: &str, message: &str) {
    use std::io::Write;
    
    let log_dir = std::path::Path::new("logs");
    let _ = std::fs::create_dir_all(log_dir);
    
    let path = log_dir.join("download_errors.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path) 
    {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = writeln!(file, "[{}] [{}] {}: {}", timestamp, download_id, phase, message);
    }
}

/// Parse progress from yt-dlp output line using robust regex patterns
fn parse_progress(line: &str) -> Option<DownloadProgress> {
    use progress::{parse_progress_line, ProgressUpdate};
    
    match parse_progress_line(line) {
        Some(ProgressUpdate::Progress { percent, speed, eta }) => {
            Some(DownloadProgress {
                id: String::new(), // Placeholder, to be filled by caller
                percent,
                speed,
                eta,
                status: "downloading".to_string(),
                filename: String::new(),
                error_message: None,
            })
        }
        Some(ProgressUpdate::Destination(filename)) => {
            Some(DownloadProgress {
                id: String::new(),
                percent: 0.0,
                speed: String::new(),
                eta: String::new(),
                status: "downloading".to_string(),
                filename,
                error_message: None,
            })
        }
        Some(ProgressUpdate::Muxing) => {
            Some(DownloadProgress {
                id: String::new(),
                percent: 99.0, // Near complete, muxing is final step
                speed: String::new(),
                eta: String::new(),
                status: "muxing".to_string(), // Distinct status for muxing phase
                filename: String::new(),
                error_message: None,
            })
        }
        Some(ProgressUpdate::Completed) => {
            Some(DownloadProgress {
                id: String::new(),
                percent: 100.0,
                speed: String::new(),
                eta: String::new(),
                status: "complete".to_string(),
                filename: String::new(),
                error_message: None,
            })
        }
        None => None,
    }
}

/// Get streaming URL for playback without downloading
/// Returns a local proxy URL that streams the video with proper auth headers
#[tauri::command]
async fn get_streaming_url(
    app: AppHandle, 
    video_url: String, 
    _quality: String,
    proxy_state: State<'_, Arc<stream_proxy::StreamProxy>>,
) -> Result<String, String> {
    let yt_dlp_path = get_sidecar_path(&app, "yt-dlp")?;

    // Use combined formats (video+audio) for streaming
    let format_selector = "22/18/best[ext=mp4][vcodec^=avc][acodec^=mp4a]/best[ext=mp4]/best";

    println!("[Streaming] Fetching URL for: {} with format: {}", video_url, format_selector);

    let mut cmd = Command::new(&yt_dlp_path);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    cmd.arg(&video_url)
        .arg("-g") // Get URL only
        .arg("-f")
        .arg(format_selector)
        .arg("--no-playlist")
        .arg("--socket-timeout")
        .arg("15")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output()
        .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("[Streaming] Error: {}", stderr);
        return Err(format!("Failed to get streaming URL: {}", stderr));
    }

    let url_output = String::from_utf8_lossy(&output.stdout);
    let streaming_url = url_output
        .lines()
        .next()
        .ok_or("No streaming URL returned")?
        .trim()
        .to_string();

    println!("[Streaming] Got URL: {}...", &streaming_url.chars().take(80).collect::<String>());

    // Set the URL in proxy state (headers will be added by proxy)
    proxy_state.set_url(streaming_url, vec![]).await;

    // Return local proxy URL for frontend to use
    let local_url = proxy_state.get_local_url();
    println!("[Streaming] Returning proxy URL: {}", local_url);
    
    Ok(local_url)
}

/// Get streaming URLs for ALL quality levels at once
/// Returns URLs for 360p, 480p, 720p, 1080p - allows instant quality switching
/// Also sets the default quality in the proxy and returns proxy URL (like original get_streaming_url)
#[tauri::command]
async fn get_all_streaming_urls(
    app: AppHandle,
    video_url: String,
    proxy_state: State<'_, Arc<stream_proxy::StreamProxy>>,
) -> Result<StreamingUrls, String> {
    let yt_dlp_path = get_sidecar_path(&app, "yt-dlp")?;

    println!("[Streaming] Fetching all quality URLs for: {}", video_url);

    // Quality levels - use PROGRESSIVE formats only (not HLS/DASH)
    // Format IDs: 18=360p, 22=720p are progressive. For others, filter protocol
    // Using protocol filter to exclude m3u8/dash and only get https direct URLs
    let qualities = [
        ("360p", "18/best[height<=360][ext=mp4][acodec!=none][protocol^=http]"),
        ("480p", "best[height<=480][ext=mp4][acodec!=none][protocol^=http]/18"),
        ("720p", "22/best[height<=720][ext=mp4][acodec!=none][protocol^=http]"),
        ("1080p", "best[height<=1080][ext=mp4][acodec!=none][protocol^=http]/22"),
    ];

    let mut urls: HashMap<String, String> = HashMap::new();
    let mut available: Vec<String> = Vec::new();

    // Fetch URLs for each quality
    for (quality, format_selector) in qualities.iter() {
        let mut cmd = Command::new(&yt_dlp_path);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);

        cmd.arg(&video_url)
            .arg("-g") // Get URL only, no download
            .arg("-f")
            .arg(format_selector)
            .arg("--no-playlist")
            .arg("--socket-timeout")
            .arg("10")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output();
        
        if let Ok(out) = output {
            if out.status.success() {
                let url_output = String::from_utf8_lossy(&out.stdout);
                if let Some(url) = url_output.lines().next() {
                    let url = url.trim().to_string();
                    if !url.is_empty() {
                        println!("[Streaming] Got {} URL", quality);
                        urls.insert(quality.to_string(), url);
                        available.push(quality.to_string());
                    }
                }
            } else {
                println!("[Streaming] {} not available for this video", quality);
            }
        }
    }

    if urls.is_empty() {
        return Err("No streaming URLs available for any quality".to_string());
    }

    // Determine default quality (prefer 480p for bandwidth, fallback to highest available)
    let default_quality = if available.contains(&"480p".to_string()) {
        "480p".to_string()
    } else if available.contains(&"360p".to_string()) {
        "360p".to_string()
    } else {
        available.first().cloned().unwrap_or("480p".to_string())
    };

    // SET THE DEFAULT URL IN PROXY (like original get_streaming_url did)
    if let Some(default_url) = urls.get(&default_quality) {
        println!("[Streaming] Setting proxy to default quality: {}", default_quality);
        proxy_state.set_url(default_url.clone(), vec![]).await;
    }

    // Get the proxy URL to return
    let proxy_url = proxy_state.get_local_url();
    println!("[Streaming] Available: {:?}, default: {}, proxy: {}", available, default_quality, proxy_url);

    Ok(StreamingUrls {
        urls,
        available,
        default_quality,
        proxy_url,
    })
}

/// Set the stream URL in the proxy - used when switching quality
/// Frontend calls this with pre-fetched URL to instantly switch quality
#[tauri::command]
async fn set_stream_url(
    url: String,
    proxy_state: State<'_, Arc<stream_proxy::StreamProxy>>,
) -> Result<String, String> {
    println!("[Streaming] Switching to new URL: {}...", &url.chars().take(60).collect::<String>());
    
    // Update the proxy with the new URL
    proxy_state.set_url(url, vec![]).await;
    
    // Return the local proxy URL (same endpoint, different content)
    Ok(proxy_state.get_local_url())
}

/// Response for start_streaming - immediate playback with single quality
#[derive(Clone, Serialize)]
pub struct StartStreamingResponse {
    pub proxy_url: String,      // USE THIS for immediate playback
    pub quality: String,        // The quality being streamed (e.g., "360p")
}

/// Response for quality availability event
#[derive(Clone, Serialize)]
pub struct QualityAvailable {
    pub quality: String,        // e.g., "720p"
}

/// Response for the unified stream_video command
#[derive(Clone, Serialize)]
pub struct StreamResponse {
    pub proxy_url: String,              // Proxy URL for playback
    pub current_quality: String,        // Quality being streamed
    pub available_qualities: Vec<String>, // All available qualities
}

/// Unified streaming command - handles initial playback AND quality switching
/// - quality = None: Start with 360p default
/// - quality = Some("720p"): Switch to specific quality (force fresh fetch)
#[tauri::command]
async fn stream_video(
    app: AppHandle,
    video_url: String,
    quality: Option<String>,
    proxy_state: State<'_, Arc<stream_proxy::StreamProxy>>,
    url_cache: State<'_, Arc<url_cache::UrlCache>>,
) -> Result<StreamResponse, String> {
    let yt_dlp_path = get_sidecar_path(&app, "yt-dlp")?;
    
    // Determine if this is an initial load or quality switch
    let is_quality_switch = quality.is_some();
    
    // Default to 360p for fastest start
    let target_quality = quality.unwrap_or_else(|| "360p".to_string());
    
    println!("[Streaming] stream_video called - url:{} quality:{} is_switch:{}", 
        &video_url[..50.min(video_url.len())], target_quality, is_quality_switch);
    
    // Get URL - force fresh if switching quality to ensure correct URL
    let streaming_url = if is_quality_switch {
        // Quality switch: ALWAYS get fresh URL to ensure correct quality
        url_cache.get_or_fetch_fresh(&yt_dlp_path, &video_url, &target_quality).await?
    } else {
        // Initial load: use cache if available
        url_cache.get_or_fetch(&yt_dlp_path, &video_url, &target_quality).await?
    };
    
    // Log the EXACT URL being set in proxy
    let url_hash: u64 = streaming_url.bytes().fold(0u64, |acc, b| acc.wrapping_add(b as u64));
    println!("[Streaming] Setting proxy URL for {} - hash:{} url:{}...", 
        target_quality, url_hash, &streaming_url[..80.min(streaming_url.len())]);
    
    // Update proxy with the new URL
    proxy_state.set_url(streaming_url, vec![]).await;
    
    // Get available qualities from cache
    let available = fetch_available_qualities_sync(&app, &video_url, &url_cache).await;
    
    println!("[Streaming] Returning - quality:{} available:{:?}", target_quality, available);
    
    Ok(StreamResponse {
        proxy_url: proxy_state.get_local_url(),
        current_quality: target_quality,
        available_qualities: available,
    })
}

/// Fetch available qualities (quick check, no full fetch)
async fn fetch_available_qualities_sync(
    app: &AppHandle,
    video_url: &str,
    url_cache: &Arc<url_cache::UrlCache>,
) -> Vec<String> {
    let _yt_dlp_path = match get_sidecar_path(app, "yt-dlp") {
        Ok(p) => p,
        Err(_) => return vec!["360p".to_string()],
    };
    
    let qualities = ["360p", "480p", "720p", "1080p"];
    let mut available = Vec::new();
    
    // Check which qualities are already cached
    for q in qualities.iter() {
        if url_cache.get(video_url, q).await.is_some() {
            available.push(q.to_string());
        }
    }
    
    // Always include 360p as it's guaranteed
    if !available.contains(&"360p".to_string()) {
        available.push("360p".to_string());
    }
    
    // Sort by resolution
    available.sort_by(|a, b| {
        let a_num: u32 = a.replace("p", "").parse().unwrap_or(0);
        let b_num: u32 = b.replace("p", "").parse().unwrap_or(0);
        a_num.cmp(&b_num)
    });
    
    available
}

/// Fetch all quality URLs in background and emit events
#[tauri::command]
async fn fetch_all_qualities(
    app: AppHandle,
    video_url: String,
    url_cache: State<'_, Arc<url_cache::UrlCache>>,
) -> Result<Vec<String>, String> {
    let yt_dlp_path = get_sidecar_path(&app, "yt-dlp")?;
    let url_cache = url_cache.inner().clone();
    
    println!("[Streaming] Fetching all qualities in background...");
    
    let qualities = ["360p", "480p", "720p", "1080p"];
    let mut available = Vec::new();
    
    for quality in qualities.iter() {
        match url_cache.get_or_fetch(&yt_dlp_path, &video_url, quality).await {
            Ok(_) => {
                available.push(quality.to_string());
                // Emit event so frontend knows this quality is ready
                let _ = app.emit("quality-ready", QualityAvailable {
                    quality: quality.to_string(),
                });
            }
            Err(e) => {
                println!("[Streaming] {} not available: {}", quality, e);
            }
        }
    }
    
    println!("[Streaming] Available qualities: {:?}", available);
    Ok(available)
}

/// Start streaming immediately with 360p (lowest latency)
/// Returns proxy URL for instant playback - player can start within 1-2 seconds
#[tauri::command]
async fn start_streaming(
    app: AppHandle,
    video_url: String,
    proxy_state: State<'_, Arc<stream_proxy::StreamProxy>>,
) -> Result<StartStreamingResponse, String> {
    let yt_dlp_path = get_sidecar_path(&app, "yt-dlp")?;

    println!("[Streaming] Starting immediate 360p stream for: {}", video_url);

    // Use 360p for fastest start - progressive format with audio
    let format_selector = "18/best[height<=360][ext=mp4][acodec!=none][protocol^=http]";

    let mut cmd = Command::new(&yt_dlp_path);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    cmd.arg(&video_url)
        .arg("-g") // Get URL only
        .arg("-f")
        .arg(format_selector)
        .arg("--no-playlist")
        .arg("--socket-timeout")
        .arg("10")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output()
        .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("[Streaming] Error getting 360p: {}", stderr);
        return Err(format!("Failed to get streaming URL: {}", stderr));
    }

    let url_output = String::from_utf8_lossy(&output.stdout);
    let streaming_url = url_output
        .lines()
        .next()
        .ok_or("No streaming URL returned")?
        .trim()
        .to_string();

    println!("[Streaming] Got 360p URL, setting in proxy...");

    // Set in proxy for immediate playback
    proxy_state.set_url(streaming_url, vec![]).await;

    Ok(StartStreamingResponse {
        proxy_url: proxy_state.get_local_url(),
        quality: "360p".to_string(),
    })
}

/// Fetch remaining quality URLs in background and emit events as each becomes available
/// Call this AFTER playback has started to progressively load quality options
#[tauri::command]
async fn fetch_remaining_qualities(
    app: AppHandle,
    video_url: String,
) -> Result<(), String> {
    let yt_dlp_path = get_sidecar_path(&app, "yt-dlp")?;

    println!("[Streaming] Fetching remaining qualities in background...");

    // Qualities to fetch (excluding 360p which is already playing)
    let qualities = [
        ("480p", "best[height<=480][ext=mp4][acodec!=none][protocol^=http]/18"),
        ("720p", "22/best[height<=720][ext=mp4][acodec!=none][protocol^=http]"),
        ("1080p", "best[height<=1080][ext=mp4][acodec!=none][protocol^=http]/22"),
    ];

    // Spawn background task to fetch each quality
    let app_clone = app.clone();
    tokio::spawn(async move {
        for (quality, format_selector) in qualities.iter() {
            let mut cmd = Command::new(&yt_dlp_path);
            #[cfg(target_os = "windows")]
            cmd.creation_flags(0x08000000);

            cmd.arg(&video_url)
                .arg("-g")
                .arg("-f")
                .arg(format_selector)
                .arg("--no-playlist")
                .arg("--socket-timeout")
                .arg("10")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            let output = cmd.output();

            if let Ok(out) = output {
                if out.status.success() {
                    let url_output = String::from_utf8_lossy(&out.stdout);
                    if let Some(url) = url_output.lines().next() {
                        let url = url.trim().to_string();
                        if !url.is_empty() {
                            println!("[Streaming] {} available", quality);
                            
                            // Emit event to frontend
                            let _ = app_clone.emit("quality-available", QualityAvailable {
                                quality: quality.to_string(),
                            });
                        }
                    }
                } else {
                    println!("[Streaming] {} not available for this video", quality);
                }
            }
        }
        println!("[Streaming] Finished fetching all qualities");
    });

    Ok(())
}

/// Switch to a different quality - fetches fresh URL if needed for reliability
/// cached_url can be provided for instant switch, but will re-fetch if it fails
#[tauri::command]
async fn switch_quality(
    app: AppHandle,
    video_url: String,
    quality: String,
    cached_url: Option<String>,
    proxy_state: State<'_, Arc<stream_proxy::StreamProxy>>,
) -> Result<String, String> {
    println!("[Streaming] Switching to quality: {}", quality);

    // Try cached URL first if provided
    if let Some(url) = cached_url {
        println!("[Streaming] Using cached URL for {}", quality);
        proxy_state.set_url(url, vec![]).await;
        return Ok(proxy_state.get_local_url());
    }

    // No cached URL or need fresh fetch
    let yt_dlp_path = get_sidecar_path(&app, "yt-dlp")?;
    
    let format_selector = match quality.as_str() {
        "360p" => "18/best[height<=360][ext=mp4][acodec!=none][protocol^=http]",
        "480p" => "best[height<=480][ext=mp4][acodec!=none][protocol^=http]/18",
        "720p" => "22/best[height<=720][ext=mp4][acodec!=none][protocol^=http]",
        "1080p" => "best[height<=1080][ext=mp4][acodec!=none][protocol^=http]/22",
        _ => "best[ext=mp4][acodec!=none][protocol^=http]",
    };

    println!("[Streaming] Fetching fresh URL for {}", quality);

    let mut cmd = Command::new(&yt_dlp_path);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    cmd.arg(&video_url)
        .arg("-g")
        .arg("-f")
        .arg(format_selector)
        .arg("--no-playlist")
        .arg("--socket-timeout")
        .arg("10")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output()
        .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to get {} URL: {}", quality, stderr));
    }

    let url_output = String::from_utf8_lossy(&output.stdout);
    let streaming_url = url_output
        .lines()
        .next()
        .ok_or("No streaming URL returned")?
        .trim()
        .to_string();

    proxy_state.set_url(streaming_url, vec![]).await;
    Ok(proxy_state.get_local_url())
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

/// Check if a format's codecs are compatible with our bundled FFmpeg build
/// Our custom FFmpeg has: H.264, HEVC, VP8, VP9 video decoders; AAC, MP3, Opus, Vorbis audio decoders
/// Missing: AV1 video decoder, MKV muxer
fn is_ffmpeg_compatible(vcodec: &str, acodec: &str, ext: &str) -> bool {
    let vcodec_lower = vcodec.to_lowercase();
    let acodec_lower = acodec.to_lowercase();
    let ext_lower = ext.to_lowercase();
    
    // Supported video codecs (our FFmpeg build includes these decoders)
    let video_ok = vcodec_lower == "none" 
        || vcodec_lower.contains("avc") 
        || vcodec_lower.contains("h264")
        || vcodec_lower.contains("hevc") 
        || vcodec_lower.contains("hev1")
        || vcodec_lower.contains("hvc1")
        || vcodec_lower.contains("vp8") 
        || vcodec_lower.contains("vp9")
        || vcodec_lower.contains("vp09");
    
    // AV1 is NOT supported - explicitly reject
    if vcodec_lower.contains("av01") || vcodec_lower.contains("av1") {
        return false;
    }
    
    // Supported audio codecs
    let audio_ok = acodec_lower == "none"
        || acodec_lower.contains("aac")
        || acodec_lower.contains("mp4a")
        || acodec_lower.contains("mp3")
        || acodec_lower.contains("opus")
        || acodec_lower.contains("vorbis")
        || acodec_lower.contains("ac3")
        || acodec_lower.contains("eac3")
        || acodec_lower.contains("flac");
    
    // Supported container formats (muxers we have)
    let ext_ok = matches!(ext_lower.as_str(), "mp4" | "m4a" | "webm" | "mov" | "mp3" | "ogg" | "3gp");
    
    video_ok && audio_ok && ext_ok
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
        let tbr = fmt["tbr"].as_f64().unwrap_or(0.0);
        
        // Filter out obviously bad stuff or storyboards
        if format_id.contains("sb") || ext == "mhtml" {
            continue;
        }
        
        // Filter out formats our bundled FFmpeg cannot mux (e.g., AV1, MKV)
        if !is_ffmpeg_compatible(&vcodec, &acodec, &ext) {
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
                tbr,
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

/// Update yt-dlp binary
#[tauri::command]
async fn update_ytdlp(app: AppHandle) -> Result<String, String> {
    let yt_dlp_path = get_sidecar_path(&app, "yt-dlp")?;
    
    println!("Updating yt-dlp at: {:?}", yt_dlp_path);
    
    // Command: yt-dlp -U
    let mut cmd = Command::new(&yt_dlp_path);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    cmd.arg("-U")
       .stdout(Stdio::piped())
       .stderr(Stdio::piped());

    let output = cmd.output()
        .map_err(|e| format!("Failed to run update command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    if !output.status.success() {
        return Err(format!("Update failed: {} {}", stdout, stderr));
    }
    
    Ok(format!("{}\n{}", stdout, stderr))
}

// ========== APP UPDATE COMMANDS ==========

#[derive(Clone, Serialize)]
struct AppUpdateInfo {
    version: String,
    notes: String,
    download_url: String,
    current_version: String,
    update_available: bool,
}

#[derive(Clone, Serialize)]
struct AppUpdateProgress {
    percent: f64,
    downloaded_bytes: u64,
    total_bytes: u64,
    status: String,
}

/// Check if app update is available by fetching latest.json from GitHub
#[tauri::command]
async fn check_app_update() -> Result<AppUpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let endpoint = "https://github.com/sahasr-aksha/alpha-tube/releases/latest/download/latest.json";
    
    // Simple HTTP GET using ureq (sync) or we can shell out to curl
    // Using std::process::Command with curl for simplicity (no new deps)
    let mut cmd = Command::new("curl");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    
    let output = cmd.args(["-sL", endpoint])
        .output()
        .map_err(|e| format!("Network error: {}", e))?;
    
    if !output.status.success() {
        return Err("Failed to fetch update info".to_string());
    }
    
    let body = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("Parse error: {}", e))?;
    
    let remote_version = json["version"].as_str().unwrap_or("0.0.0");
    let notes = json["notes"].as_str().unwrap_or("");
    let download_url = json["platforms"]["windows-x86_64"]["url"]
        .as_str()
        .unwrap_or("");
    
    // Simple version comparison (works for semver x.y.z)
    let update_available = remote_version > current_version;
    
    println!("[App Update] Current: {}, Remote: {}, Available: {}", 
             current_version, remote_version, update_available);
    
    Ok(AppUpdateInfo {
        version: remote_version.to_string(),
        notes: notes.to_string(),
        download_url: download_url.to_string(),
        current_version: current_version.to_string(),
        update_available,
    })
}

/// Download app update using aria2c for fast parallel download with progress
#[tauri::command]
async fn download_app_update(app: AppHandle, download_url: String) -> Result<String, String> {
    use std::io::{BufRead, BufReader};
    use regex::Regex;
    
    let aria2_path = get_sidecar_path(&app, "aria2c")?;
    
    // Save to app cache directory
    let cache_dir = app.path().app_cache_dir()
        .map_err(|e| format!("Failed to get cache dir: {}", e))?;
    
    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("Failed to create cache dir: {}", e))?;
    }
    
    let output_file = cache_dir.join("AlphaTube_update.exe");
    
    // Remove old update file if exists
    if output_file.exists() {
        let _ = fs::remove_file(&output_file);
    }
    
    println!("[App Update] Downloading to: {:?}", output_file);
    
    let mut cmd = Command::new(&aria2_path);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    
    cmd.arg(&download_url)
       .arg("-d").arg(&cache_dir)
       .arg("-o").arg("AlphaTube_update.exe")
       .arg("-c") // Continue if partial
       .arg("--file-allocation=none")
       .arg("--summary-interval=1")
       .arg("--max-connection-per-server=16")
       .arg("--split=16")
       .arg("--min-split-size=1M")
       .stdout(Stdio::piped())
       .stderr(Stdio::piped());
    
    let mut child = cmd.spawn()
        .map_err(|e| format!("Failed to start aria2c: {}", e))?;
    
    let stdout = child.stdout.take()
        .ok_or("Failed to capture stdout")?;
    
    let reader = BufReader::new(stdout);
    let app_handle = app.clone();
    
    // Parse aria2c output for progress
    let progress_re = Regex::new(r"\((\d+)%\)").unwrap();
    
    // Spawn thread to read output and emit progress
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    
    std::thread::spawn(move || {
        for line in reader.lines() {
            if let Ok(l) = line {
                let _ = tx.send(l);
            }
        }
    });
    
    loop {
        // Check for output
        while let Ok(line) = rx.try_recv() {
            if let Some(caps) = progress_re.captures(&line) {
                if let Some(m) = caps.get(1) {
                    if let Ok(percent) = m.as_str().parse::<f64>() {
                        let _ = app_handle.emit("app-update-progress", AppUpdateProgress {
                            percent,
                            downloaded_bytes: 0, // aria2c doesn't easily give this
                            total_bytes: 0,
                            status: "downloading".to_string(),
                        });
                    }
                }
            }
        }
        
        // Check if process finished
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    // Emit completion
                    let _ = app_handle.emit("app-update-progress", AppUpdateProgress {
                        percent: 100.0,
                        downloaded_bytes: 0,
                        total_bytes: 0,
                        status: "complete".to_string(),
                    });
                    println!("[App Update] Download complete");
                    break;
                } else {
                    return Err("Download failed".to_string());
                }
            }
            Ok(None) => {
                // Still running
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(format!("Process error: {}", e));
            }
        }
    }
    
    Ok(output_file.to_string_lossy().to_string())
}

/// Open the downloaded installer and exit app
#[tauri::command]
async fn install_app_update(installer_path: String) -> Result<(), String> {
    println!("[App Update] Launching installer: {}", installer_path);
    
    #[cfg(target_os = "windows")]
    {
        // Use cmd /C start to launch installer in background
        Command::new("cmd")
            .creation_flags(0x08000000)
            .args(["/C", "start", "", &installer_path])
            .spawn()
            .map_err(|e| format!("Failed to launch installer: {}", e))?;
    }
    
    // Exit current app to allow installer to replace files
    std::process::exit(0);
}

async fn download_video_internal(
    app: AppHandle,
    options: DownloadOptions,
    state: tauri::State<'_, DownloadManager>,
    _is_resume: bool, // Not used directly, but implicit in pipeline if implemented
) -> Result<String, String> {
    let download_id = options.id.clone();
    let should_cancel = Arc::new(AtomicBool::new(false));
    
    // Validate dependencies
    // download_pipeline checks them, but we want early fail?
    // download_manager::download_pipeline handles checks.
    
    // REGISTER DOWNLOAD IN STATE
    {
        let mut manager = state.lock().unwrap();
        manager.insert(download_id.clone(), DownloadState {
            options: options.clone(),
            should_cancel: should_cancel.clone(),
            paused: false,
            current_filename: None,
        });
    }
    
    // Run pipeline
    // We match the result to handle specific errors or completion
    match download_manager::download_pipeline(app.clone(), options, should_cancel).await {
        Ok(path) => {
            // Success
            // Remove from state
            {
                let mut manager = state.lock().unwrap();
                manager.remove(&download_id);
            }
            
            let _ = app.emit("download-progress", DownloadProgress {
                id: download_id,
                percent: 100.0,
                speed: String::new(),
                eta: String::new(),
                status: "complete".to_string(),
                filename: path.clone(),
                error_message: None,
            });
            Ok(format!("Download complete: {}", path))
        },
        Err(e) => {
            // Remove from state
            {
                let mut manager = state.lock().unwrap();
                manager.remove(&download_id);
            }
            
            match e {
                download_manager::DownloadError::Cancelled => {
                     let _ = app.emit("download-progress", DownloadProgress {
                        id: download_id,
                        percent: 0.0,
                        speed: String::new(),
                        eta: String::new(),
                        status: "cancelled".to_string(),
                        filename: String::new(),
                        error_message: None,
                    });
                    Ok("Download cancelled".to_string())
                },
                _ => {
                    let user_msg = e.user_message();
                    let _ = app.emit("download-progress", DownloadProgress {
                        id: download_id,
                        percent: 0.0,
                        speed: String::new(),
                        eta: String::new(),
                        status: "error".to_string(),
                        filename: String::new(),
                        error_message: Some(user_msg.clone()),
                    });
                    Err(format!("Download failed: {}", user_msg))
                }
            }
        }
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
    // Current architecture doesn't support pausing nicely (aria2 parallel).
    // We treat pause as cancel, but maybe we can signal frontend that it's cancelled
    // OR we just say "Pause not supported, please cancel"
    // OR we just cancel it.
    // For now, let's just error out to prompt usage of Cancel.
    // Err("Pause is not supported in this beta build. Please cancel and restart.".to_string())
    
    // Actually, user might want to stop network usage. Cancel does that.
    
    cancel_download(app, id, state).await.map(|_| "Download cancelled (Pause not supported)".to_string())
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
        error_message: None,
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
async fn get_local_video_metadata(app: AppHandle, path: String) -> Result<VideoMetadata, String> {
    let ffprobe_path = get_sidecar_path(&app, "ffprobe")?;

    let mut cmd = Command::new(&ffprobe_path);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    let output = cmd.args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams", // Added streams
            &path,
        ])
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    if !output.status.success() {
        return Err("ffprobe failed".to_string());
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse ffprobe output: {}", e))?;

    let format = &json["format"];
    let duration = format["duration"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
    
    // Find first video stream for resolution
    let streams = json["streams"].as_array().ok_or("No streams found")?;
    let video_stream = streams.iter().find(|s| s["codec_type"] == "video");
    
    let width = video_stream.and_then(|s| s["width"].as_u64()).unwrap_or(0) as u32;
    let height = video_stream.and_then(|s| s["height"].as_u64()).unwrap_or(0) as u32;
    let codec = video_stream.and_then(|s| s["codec_name"].as_str()).unwrap_or("unknown").to_string();

    // Extract audio tracks
    let mut audio_tracks = Vec::new();
    for stream in streams.iter().filter(|s| s["codec_type"] == "audio") {
        let lang = stream["tags"]["language"].as_str().unwrap_or("unknown");
        let title = stream["tags"]["title"].as_str().unwrap_or(lang);
        let codec = stream["codec_name"].as_str().unwrap_or("");
        
        let label = if title != "unknown" { 
            title.to_string() 
        } else { 
            format!("{} ({})", lang, codec)
        };
        
        audio_tracks.push(label);
    }
    
    let file_size = format["size"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    
    Ok(VideoMetadata {
        duration,
        width,
        height,
        codec,
        file_size,
        audio_tracks,
    })
}

/// Prepare HLS stream for local video if it has multiple audio tracks
/// Now spawns ffmpeg asynchronously and returns early once manifest is ready
#[tauri::command]
async fn prepare_hls_stream(
    app: AppHandle, 
    video_path: String,
    playback_state: State<'_, PlaybackManager>,
) -> Result<String, String> {
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
    let stream_hash = format!("{:x}", hash);
    let stream_dir = data_dir.join(&stream_hash);

    if !stream_dir.exists() {
        fs::create_dir_all(&stream_dir).map_err(|e| format!("Failed to create stream dir: {}", e))?;
    }

    let master_playlist_path = stream_dir.join("master.m3u8");
    let init_file_path = stream_dir.join("init.mp4");

    let marker_path = stream_dir.join("stream.done");

    // 3. Check if HLS already exists AND is complete (cached)
    if master_playlist_path.exists() && init_file_path.exists() && marker_path.exists() {
        println!("HLS cache found and complete at {:?}", master_playlist_path);
        return Ok(master_playlist_path.to_string_lossy().to_string());
    }

    // If cache exists but no marker, it's incomplete/corrupt -> delete it
    if stream_dir.exists() {
        println!("Found incomplete HLS cache, cleaning up...");
        let _ = fs::remove_dir_all(&stream_dir);
        let _ = fs::create_dir_all(&stream_dir);
    }

    // 4. Check if transcoding is already in progress for this video
    {
        let manager = playback_state.lock().unwrap();
        if manager.contains_key(&stream_hash) {
            println!("HLS generation already in progress for this video, waiting...");
            // Fall through to the polling loop below
        }
    }

    // 5. Spawn ffmpeg asynchronously
    let ffmpeg_path = get_sidecar_path(&app, "ffmpeg")?;
    
    println!("Starting ffmpeg transmux (async)...");
    let mut ffmpeg_cmd = Command::new(&ffmpeg_path);
    #[cfg(target_os = "windows")]
    ffmpeg_cmd.creation_flags(0x08000000);

    let mut child = ffmpeg_cmd
        .current_dir(&stream_dir) // Run inside stream folder so relative paths work in manifest
        .arg("-i")
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
        .arg("4") // 4s segments for faster startup
        .arg("-hls_list_size")
        .arg("0")
        .arg("-hls_playlist_type")
        .arg("event") // Tell player this is a growing stream
        .arg("-hls_segment_type")
        .arg("fmp4")
        .arg("-hls_fmp4_init_filename")
        .arg("init.mp4") // Relative path
        .arg("-hls_segment_filename")
        .arg("seg_%03d.m4s") // Relative path
        .arg("master.m3u8") // Relative output path
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn ffmpeg: {}", e))?;

    let child_pid = child.id();
    let stream_hash_clone = stream_hash.clone();
    let app_handle = app.clone();
    let marker_path_clone = marker_path.clone();

    // Spawn a background thread to wait for the process and handle cleanup/marking
    std::thread::spawn(move || {
        let status = child.wait();
        
        // Remove from manager when done
        let manager_state: State<PlaybackManager> = app_handle.state();
        {
             let mut manager = manager_state.lock().unwrap();
             manager.remove(&stream_hash_clone);
        }

        match status {
            Ok(s) => {
                if s.success() {
                    println!("FFmpeg finished successfully. Marking stream as done.");
                    let _ = fs::File::create(marker_path_clone);
                } else {
                    println!("FFmpeg finished with error.");
                }
            }
            Err(e) => println!("Failed to wait on ffmpeg child: {}", e),
        }
    });

    // 6. Register in playback manager (for cancellation)
    {
        let mut manager = playback_state.lock().unwrap();
        manager.insert(stream_hash.clone(), PlaybackState {
            child_pid,
            video_path: video_path.clone(),
            stream_hash: stream_hash.clone(),
        });
    }

    // 7. Poll for manifest + init file to be ready (timeout 30s)
    let poll_interval = Duration::from_millis(200);
    let max_wait = Duration::from_secs(30);
    let start_time = std::time::Instant::now();

    loop {
        // Check if both manifest and init file exist
        if master_playlist_path.exists() && init_file_path.exists() {
            // Also check if there's at least one segment
            let first_segment = stream_dir.join("seg_000.m4s");
            if first_segment.exists() {
                println!("HLS stream ready for playback (ffmpeg continues in background)");
                return Ok(master_playlist_path.to_string_lossy().to_string());
            }
        }

        if start_time.elapsed() > max_wait {
            // Timeout - cleanup and return error
            {
                let mut manager = playback_state.lock().unwrap();
                if let Some(state) = manager.remove(&stream_hash) {
                    // Kill the process
                    #[cfg(target_os = "windows")]
                    {
                        let _ = Command::new("taskkill")
                            .creation_flags(0x08000000)
                            .args(["/F", "/T", "/PID", &state.child_pid.to_string()])
                            .output();
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        let _ = Command::new("kill")
                            .arg(state.child_pid.to_string())
                            .output();
                    }
                }
            }
            return Err("Timeout waiting for HLS stream to be ready".to_string());
        }

        tokio::time::sleep(poll_interval).await;
    }
}



/// Cancel/stop HLS streaming process for a video (or specific audio track)
#[tauri::command]
async fn cancel_stream(
    video_path: String,
    audio_index: Option<usize>,
    playback_state: State<'_, PlaybackManager>,
) -> Result<String, String> {
    // Hash to find the stream
    let mut hasher = DefaultHasher::new();
    video_path.hash(&mut hasher);
    if let Some(idx) = audio_index {
        idx.hash(&mut hasher);
    }
    let hash = hasher.finish();
    let stream_hash = format!("{:x}", hash);

    let mut manager = playback_state.lock().unwrap();
    
    if let Some(state) = manager.remove(&stream_hash) {
        println!("Stopping HLS generation for: {}", video_path);
        
        // Kill the ffmpeg process
        #[cfg(target_os = "windows")]
        {
            let output = Command::new("taskkill")
                .creation_flags(0x08000000)
                .args(["/F", "/T", "/PID", &state.child_pid.to_string()])
                .output();
            
            match output {
                Ok(o) => {
                    if o.status.success() {
                        println!("Successfully killed ffmpeg process {}", state.child_pid);
                    } else {
                        println!("Process {} may have already exited", state.child_pid);
                    }
                }
                Err(e) => println!("Failed to kill process: {}", e),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = Command::new("kill")
                .arg(state.child_pid.to_string())
                .output();
        }
        
        Ok("Stream cancelled".to_string())
    } else {
        // Process might have finished naturally, that's fine
        Ok("No active stream for this video".to_string())
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

/// Check for and apply yt-dlp updates silently
/// Only runs when no downloads are in progress
/// Emits events for future frontend notification:
/// - "ytdlp-update-status" with { status: "checking" | "updating" | "complete" | "error" | "skipped", message?: string }
fn check_and_update_ytdlp(app: &AppHandle) {
    println!("[yt-dlp Updater] Checking for updates...");
    
    // Check if any downloads are active
    let download_manager: tauri::State<'_, DownloadManager> = app.state();
    {
        let manager = download_manager.lock().unwrap();
        if !manager.is_empty() {
            println!("[yt-dlp Updater] Downloads in progress, skipping update check");
            let _ = app.emit("ytdlp-update-status", serde_json::json!({
                "status": "skipped",
                "message": "Downloads in progress"
            }));
            return;
        }
    }
    
    // Get yt-dlp path
    let yt_dlp_path = match get_sidecar_path(app, "yt-dlp") {
        Ok(path) => path,
        Err(e) => {
            println!("[yt-dlp Updater] Failed to get yt-dlp path: {}", e);
            let _ = app.emit("ytdlp-update-status", serde_json::json!({
                "status": "error",
                "message": format!("Failed to get yt-dlp path: {}", e)
            }));
            return;
        }
    };
    
    // Emit checking status
    let _ = app.emit("ytdlp-update-status", serde_json::json!({
        "status": "checking"
    }));
    
    // Run yt-dlp -U silently
    let mut cmd = Command::new(&yt_dlp_path);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    
    cmd.arg("-U");
    
    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            if output.status.success() {
                // Check if actually updated or already up-to-date
                let message = stdout.trim().to_string();
                let is_updated = message.contains("Updated") || message.contains("Updating");
                
                println!("[yt-dlp Updater] {}", message);
                let _ = app.emit("ytdlp-update-status", serde_json::json!({
                    "status": "complete",
                    "message": message,
                    "updated": is_updated
                }));
            } else {
                let error_msg = stderr.trim().to_string();
                println!("[yt-dlp Updater] Update failed: {}", error_msg);
                let _ = app.emit("ytdlp-update-status", serde_json::json!({
                    "status": "error",
                    "message": error_msg
                }));
            }
        }
        Err(e) => {
            println!("[yt-dlp Updater] Failed to run update: {}", e);
            let _ = app.emit("ytdlp-update-status", serde_json::json!({
                "status": "error",
                "message": format!("Failed to run update: {}", e)
            }));
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
            // Initialize Playback Manager State (for HLS ffmpeg processes)
            app.manage(std::sync::Mutex::new(std::collections::HashMap::<String, PlaybackState>::new()));
            
            // Initialize Stream Proxy for video playback
            let stream_proxy = Arc::new(stream_proxy::StreamProxy::new(9876));
            app.manage(stream_proxy.clone());
            
            // Initialize URL Cache for quality switching
            let url_cache = url_cache::UrlCache::new();
            app.manage(url_cache);
            
            // Start proxy server in background
            tauri::async_runtime::spawn(async move {
                stream_proxy::start_proxy_server(stream_proxy).await;
            });

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

            // Start background yt-dlp update checker
            let app_handle_ytdlp = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Wait 30 seconds after startup to allow app to stabilize
                tokio::time::sleep(Duration::from_secs(30)).await;
                check_and_update_ytdlp(&app_handle_ytdlp);
                
                // Then check every 12 hours
                loop {
                    tokio::time::sleep(Duration::from_secs(12 * 60 * 60)).await;
                    check_and_update_ytdlp(&app_handle_ytdlp);
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
            cancel_stream,
            get_app_config, 
            set_download_dir, 
            cancel_download, 
            delete_file, 
            reveal_file_in_explorer,
            search::search_videos,
            playlist::get_playlist_metadata,
            playlist::get_video_formats,
            get_streaming_url,
            get_all_streaming_urls,
            set_stream_url,
            start_streaming,
            fetch_remaining_qualities,
            switch_quality,
            stream_video,
            fetch_all_qualities,
            update_ytdlp,
            check_app_update,
            download_app_update,
            install_app_update
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





