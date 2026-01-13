//! Playlist handling module for Alpha Tube
//!
//! Handles fetching and processing playlist metadata with individual video entries.
//! Provides a dedicated Tauri command for playlist-specific operations.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use tauri::{AppHandle, Manager};

// Re-export VideoFormat from lib for playlist entries
// We'll define a simplified version here to avoid circular dependencies
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PlaylistVideoFormat {
    pub format_id: String,
    pub ext: String,
    pub resolution: String,
    pub fps: f64,
    pub filesize: u64,
    pub vcodec: String,
    pub acodec: String,
    pub note: String,
}

/// Individual video entry within a playlist
#[derive(Clone, Serialize, Debug)]
pub struct PlaylistVideoEntry {
    /// Unique video ID (from YouTube)
    pub id: String,
    /// Video title
    pub title: String,
    /// Thumbnail URL
    pub thumbnail_url: String,
    /// Video duration in seconds
    pub duration: f64,
    /// Direct video URL for downloading
    pub url: String,
    /// Position in playlist (1-indexed)
    pub index: usize,
    /// Available quality formats
    pub formats: Vec<PlaylistVideoFormat>,
}

/// Full playlist metadata response
#[derive(Clone, Serialize, Debug)]
pub struct PlaylistMetadataResponse {
    /// Playlist title
    pub title: String,
    /// Playlist thumbnail URL
    pub thumbnail_url: String,
    /// Total video count in playlist
    pub video_count: usize,
    /// Unique playlist ID
    pub playlist_id: String,
    /// Individual video entries (limited to first 50)
    pub videos: Vec<PlaylistVideoEntry>,
}

/// Options for downloading selected playlist videos
#[derive(Deserialize, Clone, Serialize, Debug)]
pub struct PlaylistVideoDownload {
    /// Video URL
    pub url: String,
    /// Specific format ID to download (optional)
    pub format_id: Option<String>,
    /// Quality preset (e.g., "720p", "1080p", "best")
    pub quality: String,
    /// Position in playlist (1-indexed)
    pub index: usize,
    /// Video title (for progress tracking)
    pub title: String,
    /// Thumbnail URL (for UI display)
    pub thumbnail_url: String,
    /// Duration in seconds
    pub duration: f64,
}

/// Options for downloading multiple videos from a playlist
#[derive(Deserialize, Clone, Serialize, Debug)]
pub struct PlaylistDownloadOptions {
    /// Unique ID for this batch download
    pub id: String,
    /// Playlist ID
    pub playlist_id: String,
    /// Playlist title (used for folder creation)
    pub playlist_title: String,
    /// Selected videos to download
    pub videos: Vec<PlaylistVideoDownload>,
    /// Output directory
    pub output_path: String,
}

/// Enhanced download progress with playlist context
#[derive(Clone, Serialize, Debug)]
pub struct PlaylistDownloadProgress {
    /// Unique download ID
    pub id: String,
    /// Download percent (0-100)
    pub percent: f64,
    /// Download speed string
    pub speed: String,
    /// Estimated time remaining
    pub eta: String,
    /// Status: "downloading", "processing", "complete", "error", "paused"
    pub status: String,
    /// Current filename being downloaded
    pub filename: String,
    /// Playlist ID this download belongs to (None for single videos)
    pub playlist_id: Option<String>,
    /// Playlist title
    pub playlist_title: Option<String>,
    /// Current video index in playlist (1-indexed)
    pub playlist_index: Option<usize>,
    /// Total videos in this playlist batch
    pub playlist_total: Option<usize>,
    /// Video title
    pub video_title: Option<String>,
    /// Thumbnail URL
    pub thumbnail_url: Option<String>,
    /// Duration in seconds
    pub duration: Option<f64>,
}

/// Get the path to the sidecar binary (duplicated from lib.rs to avoid circular deps)
fn get_sidecar_path(app: &AppHandle, name: &str) -> Result<std::path::PathBuf, String> {
    let suffixes = [
        "-x86_64-pc-windows-gnu.exe",
        "-x86_64-pc-windows-msvc.exe",
        ".exe",
        "",
    ];
    
    let mut search_paths: Vec<std::path::PathBuf> = Vec::new();
    
    if let Ok(resource_path) = app.path().resource_dir() {
        search_paths.push(resource_path.join("bin"));
    }
    
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            search_paths.push(exe_dir.to_path_buf());
            search_paths.push(exe_dir.join("bin"));
            
            let dev_bin_path = exe_dir.parent()
                .and_then(|p| p.parent())
                .map(|p| p.join("bin"));
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
    
    Err(format!(
        "Sidecar '{}' not found. Searched in: {:?}",
        name,
        search_paths
    ))
}

/// Parse a single video entry from yt-dlp JSON
fn parse_video_entry(entry: &serde_json::Value, index: usize) -> Option<PlaylistVideoEntry> {
    let id = entry["id"].as_str().unwrap_or("").to_string();
    let title = entry["title"].as_str().unwrap_or("Unknown Title").to_string();
    let duration = entry["duration"].as_f64().unwrap_or(0.0);
    
    // Build video URL from ID
    let url = if !id.is_empty() {
        format!("https://www.youtube.com/watch?v={}", id)
    } else {
        entry["url"].as_str().unwrap_or("").to_string()
    };
    
    // Get thumbnail
    let thumbnail_url = entry["thumbnail"].as_str()
        .or_else(|| entry["thumbnails"].as_array()
            .and_then(|t| t.last())
            .and_then(|t| t["url"].as_str()))
        .unwrap_or("")
        .to_string();
    
    // Parse formats if available
    let formats: Vec<PlaylistVideoFormat> = entry["formats"].as_array()
        .map(|fmts| {
            fmts.iter()
                .filter_map(|fmt| {
                    let format_id = fmt["format_id"].as_str()?.to_string();
                    let ext = fmt["ext"].as_str().unwrap_or("").to_string();
                    let vcodec = fmt["vcodec"].as_str().unwrap_or("none").to_string();
                    let acodec = fmt["acodec"].as_str().unwrap_or("none").to_string();
                    
                    // Skip storyboards and mhtml formats
                    if format_id.contains("sb") || ext == "mhtml" {
                        return None;
                    }
                    
                    // Only include video formats or audio-only
                    if vcodec == "none" && acodec == "none" {
                        return None;
                    }
                    
                    let resolution = fmt["resolution"].as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| {
                            if let (Some(w), Some(h)) = (fmt["width"].as_u64(), fmt["height"].as_u64()) {
                                format!("{}x{}", w, h)
                            } else if vcodec == "none" {
                                "audio only".to_string()
                            } else {
                                "unknown".to_string()
                            }
                        });
                    
                    Some(PlaylistVideoFormat {
                        format_id,
                        ext,
                        resolution,
                        fps: fmt["fps"].as_f64().unwrap_or(0.0),
                        filesize: fmt["filesize"].as_u64()
                            .or(fmt["filesize_approx"].as_u64())
                            .unwrap_or(0),
                        vcodec,
                        acodec,
                        note: fmt["format_note"].as_str().unwrap_or("").to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    
    if id.is_empty() && url.is_empty() {
        return None;
    }
    
    Some(PlaylistVideoEntry {
        id,
        title,
        thumbnail_url,
        duration,
        url,
        index,
        formats,
    })
}

/// Fetch full playlist metadata with individual video entries
#[tauri::command]
pub async fn get_playlist_metadata(
    app: AppHandle,
    url: String,
) -> Result<PlaylistMetadataResponse, String> {
    let yt_dlp_path = get_sidecar_path(&app, "yt-dlp")?;
    
    println!("[playlist] Fetching playlist metadata for: {}", url);
    
    // Fetch playlist with individual video details (first 50)
    let mut cmd = Command::new(&yt_dlp_path);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    
    cmd.arg(&url)
        .arg("-J")                    // JSON output
        .arg("--playlist-items")
        .arg("1:50")                  // Limit to first 50 videos
        .arg("--flat-playlist")       // Don't fetch individual video formats yet
        .arg("--socket-timeout")
        .arg("15")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    
    let output = cmd.output()
        .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("[playlist] yt-dlp error: {}", stderr);
        return Err(format!("yt-dlp failed: {}", stderr));
    }
    
    let json_str = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;
    
    // Verify this is a playlist
    let playlist_type = json["_type"].as_str().unwrap_or("");
    if playlist_type != "playlist" {
        return Err("URL is not a playlist".to_string());
    }
    
    let title = json["title"].as_str().unwrap_or("Unknown Playlist").to_string();
    let playlist_id = json["id"].as_str().unwrap_or("").to_string();
    
    let video_count = json["playlist_count"].as_u64()
        .or_else(|| json["entries"].as_array().map(|a| a.len() as u64))
        .unwrap_or(0) as usize;
    
    let thumbnail_url = json["thumbnails"].as_array()
        .and_then(|t| t.last())
        .and_then(|t| t["url"].as_str())
        .unwrap_or("")
        .to_string();
    
    // Parse video entries
    let entries = json["entries"].as_array()
        .map(|arr| arr.to_vec())
        .unwrap_or_default();
    
    println!("[playlist] Found {} entries (total: {})", entries.len(), video_count);
    
    // Convert to our video entries (with basic info from flat-playlist)
    let videos: Vec<PlaylistVideoEntry> = entries.iter()
        .enumerate()
        .filter_map(|(i, entry)| {
            let id = entry["id"].as_str().unwrap_or("").to_string();
            let title = entry["title"].as_str().unwrap_or("Unknown Title").to_string();
            let duration = entry["duration"].as_f64().unwrap_or(0.0);
            
            let url = if !id.is_empty() {
                format!("https://www.youtube.com/watch?v={}", id)
            } else {
                entry["url"].as_str().unwrap_or("").to_string()
            };
            
            let thumbnail_url = entry["thumbnails"].as_array()
                .and_then(|t| t.first())
                .and_then(|t| t["url"].as_str())
                .unwrap_or("")
                .to_string();
            
            if id.is_empty() && url.is_empty() {
                return None;
            }
            
            Some(PlaylistVideoEntry {
                id,
                title,
                thumbnail_url,
                duration,
                url,
                index: i + 1,  // 1-indexed
                formats: vec![],  // Will be fetched on demand
            })
        })
        .collect();
    
    println!("[playlist] Parsed {} video entries", videos.len());
    
    Ok(PlaylistMetadataResponse {
        title,
        thumbnail_url,
        video_count,
        playlist_id,
        videos,
    })
}

/// Fetch formats for a specific video (called on-demand for quality selection)
#[tauri::command]
pub async fn get_video_formats(
    app: AppHandle,
    url: String,
) -> Result<Vec<PlaylistVideoFormat>, String> {
    let yt_dlp_path = get_sidecar_path(&app, "yt-dlp")?;
    
    let mut cmd = Command::new(&yt_dlp_path);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    
    cmd.arg(&url)
        .arg("--dump-json")
        .arg("--no-playlist")
        .arg("--socket-timeout")
        .arg("10")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    
    let output = cmd.output()
        .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp failed: {}", stderr));
    }
    
    let json_str = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;
    
    let formats: Vec<PlaylistVideoFormat> = json["formats"].as_array()
        .map(|fmts| {
            fmts.iter()
                .filter_map(|fmt| {
                    let format_id = fmt["format_id"].as_str()?.to_string();
                    let ext = fmt["ext"].as_str().unwrap_or("").to_string();
                    let vcodec = fmt["vcodec"].as_str().unwrap_or("none").to_string();
                    let acodec = fmt["acodec"].as_str().unwrap_or("none").to_string();
                    
                    if format_id.contains("sb") || ext == "mhtml" {
                        return None;
                    }
                    
                    if vcodec == "none" && acodec == "none" {
                        return None;
                    }
                    
                    let resolution = fmt["resolution"].as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| {
                            if let (Some(w), Some(h)) = (fmt["width"].as_u64(), fmt["height"].as_u64()) {
                                format!("{}x{}", w, h)
                            } else if vcodec == "none" {
                                "audio only".to_string()
                            } else {
                                "unknown".to_string()
                            }
                        });
                    
                    Some(PlaylistVideoFormat {
                        format_id,
                        ext,
                        resolution,
                        fps: fmt["fps"].as_f64().unwrap_or(0.0),
                        filesize: fmt["filesize"].as_u64()
                            .or(fmt["filesize_approx"].as_u64())
                            .unwrap_or(0),
                        vcodec,
                        acodec,
                        note: fmt["format_note"].as_str().unwrap_or("").to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    
    Ok(formats)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_video_entry() {
        let json_str = r#"{
            "id": "dQw4w9WgXcQ",
            "title": "Test Video",
            "duration": 212.0,
            "thumbnail": "https://example.com/thumb.jpg"
        }"#;
        
        let value: serde_json::Value = serde_json::from_str(json_str).unwrap();
        let entry = parse_video_entry(&value, 1);
        
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.id, "dQw4w9WgXcQ");
        assert_eq!(entry.title, "Test Video");
        assert_eq!(entry.index, 1);
        assert!(entry.url.contains("dQw4w9WgXcQ"));
    }
}
