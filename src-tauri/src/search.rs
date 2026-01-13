// Search module for video search functionality
// Uses yt-dlp to search YouTube and return video results

use serde::Serialize;
use std::process::{Command, Stdio};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use tauri::AppHandle;

/// Represents a video search result
#[derive(Clone, Serialize)]
pub struct VideoSearchResult {
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub video_url: String,
    pub duration: Option<String>,
    pub uploader: Option<String>,
    pub view_count: Option<u64>,
}

/// Build search URL based on platform
/// 
/// Supported platforms:
/// - ytsearch: YouTube (default)
/// - scsearch: SoundCloud
/// - bilisearch: Bilibili
/// - nicosearch: Niconico
/// - dailymotion: Dailymotion
/// - gvsearch: Google Video
/// - yvsearch: Yahoo Screen
fn build_search_url(query: &str, platform: &str) -> String {
    let encoded_query = urlencoding::encode(query);
    
    match platform {
        "ytsearch" | "youtube" | "" => {
            // YouTube: Use results page with upload date sorting
            format!(
                "https://www.youtube.com/results?search_query={}&sp=CAMSAhAB",
                encoded_query
            )
        }
        "scsearch" => format!("scsearch20:{}", query),
        "bilisearch" => format!("bilisearch20:{}", query),
        "nicosearch" => format!("nicosearch20:{}", query),
        "dailymotion" => format!("https://www.dailymotion.com/search/{}/videos", encoded_query),
        "gvsearch" => format!("gvsearch20:{}", query),
        "yvsearch" => format!("yvsearch20:{}", query),
        _ => format!("ytsearch20:{}", query), // Fallback to YouTube
    }
}

/// Search for videos using yt-dlp
/// 
/// Supports multiple platforms via the `platform` parameter.
/// Returns title, thumbnail, and video URL for each result.
#[tauri::command]
pub async fn search_videos(
    app: AppHandle,
    query: String,
    platform: String,
    page: usize,
    page_size: usize,
    exclude_shorts: bool,
) -> Result<Vec<VideoSearchResult>, String> {
    let yt_dlp_path = super::get_sidecar_path(&app, "yt-dlp")?;

    if query.is_empty() {
        return Ok(Vec::new());
    }

    // Build search URL based on platform
    let search_url = build_search_url(&query, &platform);

    println!("[Search] Platform: {}, URL: {}, Exclude Shorts: {}", platform, search_url, exclude_shorts);

    // Calculate pagination
    let start = (page - 1) * page_size + 1;
    let end = page * page_size;

    // Run yt-dlp with flat-playlist and dump-json
    let mut cmd = Command::new(&yt_dlp_path);
    
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    cmd.args(&[
        "--flat-playlist",
        "--dump-json",
        "--playlist-start",
        &start.to_string(),
        "--playlist-end",
        &end.to_string(),
        "--no-download",
        "--socket-timeout",
        "15",
    ]);

    // Add shorts exclusion filter if enabled (YouTube only)
    if exclude_shorts {
        cmd.args(&["--match-filter", "original_url!*=/shorts/"]);
    }

    cmd.arg(&search_url)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    println!("[Search] Executing yt-dlp search for: {}", query);
    
    let output = cmd.output()
        .map_err(|e| format!("Failed to run yt-dlp for search: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("[Search] yt-dlp search failed: {}", stderr);
        return Err(format!("yt-dlp search failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();

    // Each line is a separate JSON object for one video
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            let title = json.get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();

            let thumbnail_url = json.get("thumbnail")
                .or_else(|| json.get("thumbnails")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.last())
                    .and_then(|t| t.get("url")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // Get video URL - prefer "url" field, fallback to constructing from "id"
            let video_url = json.get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    json.get("webpage_url")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .or_else(|| {
                    // Fallback: construct YouTube URL from ID if platform is YouTube
                    json.get("id")
                        .and_then(|v| v.as_str())
                        .map(|id| {
                            if id.starts_with("http") {
                                id.to_string()
                            } else if platform == "ytsearch" || platform == "youtube" || platform.is_empty() {
                                format!("https://www.youtube.com/watch?v={}", id)
                            } else {
                                id.to_string() // For other platforms, yt-dlp should provide full URL
                            }
                        })
                })
                .unwrap_or_default();

            let duration = json.get("duration_string")
                .or_else(|| json.get("duration").map(|d| d))
                .and_then(|v| {
                    if v.is_string() {
                        v.as_str().map(|s| s.to_string())
                    } else if let Some(secs) = v.as_u64() {
                        Some(format!("{}:{:02}", secs / 60, secs % 60))
                    } else {
                        None
                    }
                });

            let uploader = json.get("uploader")
                .or_else(|| json.get("channel"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let view_count = json.get("view_count")
                .and_then(|v| v.as_u64());

            results.push(VideoSearchResult {
                title,
                thumbnail_url,
                video_url,
                duration,
                uploader,
                view_count,
            });
        }
    }

    println!("[Search] Found {} results", results.len());
    Ok(results)
}
