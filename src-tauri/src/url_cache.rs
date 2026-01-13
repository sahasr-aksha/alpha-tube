// url_cache.rs - URL caching with TTL for reliable quality switching

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use std::process::{Command, Stdio};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// TTL for cached URLs - 4 hours (YouTube URLs expire around 6 hours)
const URL_TTL_SECS: u64 = 4 * 60 * 60;

/// A cached streaming URL with timestamp
#[derive(Clone, Debug)]
pub struct CachedUrl {
    pub url: String,
    pub fetched_at: Instant,
}

impl CachedUrl {
    pub fn new(url: String) -> Self {
        Self {
            url,
            fetched_at: Instant::now(),
        }
    }

    pub fn is_fresh(&self) -> bool {
        self.fetched_at.elapsed().as_secs() < URL_TTL_SECS
    }
}

/// Thread-safe URL cache for all video qualities
pub struct UrlCache {
    /// Cache key format: "{video_url}:{quality}" -> CachedUrl
    cache: RwLock<HashMap<String, CachedUrl>>,
}

impl UrlCache {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            cache: RwLock::new(HashMap::new()),
        })
    }

    /// Get cache key for a video+quality combination
    fn cache_key(video_url: &str, quality: &str) -> String {
        format!("{}:{}", video_url, quality)
    }

    /// Get URL from cache if fresh, otherwise return None
    pub async fn get(&self, video_url: &str, quality: &str) -> Option<String> {
        let key = Self::cache_key(video_url, quality);
        let cache = self.cache.read().await;
        
        if let Some(cached) = cache.get(&key) {
            if cached.is_fresh() {
                return Some(cached.url.clone());
            }
        }
        None
    }

    /// Store URL in cache with current timestamp
    pub async fn set(&self, video_url: &str, quality: &str, url: String) {
        let key = Self::cache_key(video_url, quality);
        let mut cache = self.cache.write().await;
        cache.insert(key, CachedUrl::new(url));
    }

    /// Get or fetch URL - the main entry point
    /// Returns cached URL if fresh, otherwise fetches from yt-dlp
    /// Set force_fresh = true to ALWAYS fetch from yt-dlp (for quality switching)
    pub async fn get_or_fetch(
        &self,
        yt_dlp_path: &std::path::Path,
        video_url: &str,
        quality: &str,
    ) -> Result<String, String> {
        self.get_or_fetch_internal(yt_dlp_path, video_url, quality, false).await
    }
    
    /// Force fresh fetch - always gets a new URL from yt-dlp
    pub async fn get_or_fetch_fresh(
        &self,
        yt_dlp_path: &std::path::Path,
        video_url: &str,
        quality: &str,
    ) -> Result<String, String> {
        self.get_or_fetch_internal(yt_dlp_path, video_url, quality, true).await
    }
    
    async fn get_or_fetch_internal(
        &self,
        yt_dlp_path: &std::path::Path,
        video_url: &str,
        quality: &str,
        force_fresh: bool,
    ) -> Result<String, String> {
        // Check cache first (unless force_fresh)
        if !force_fresh {
            if let Some(url) = self.get(video_url, quality).await {
                // Log URL hash to verify uniqueness
                let hash: u64 = url.bytes().fold(0u64, |acc, b| acc.wrapping_add(b as u64));
                println!("[UrlCache] Cache HIT for {} - hash:{} url:{}...", quality, hash, &url[..60.min(url.len())]);
                return Ok(url);
            }
        } else {
            println!("[UrlCache] FORCE FRESH for {} - bypassing cache", quality);
        }

        // Fetch fresh
        println!("[UrlCache] Cache MISS for {}, fetching fresh from yt-dlp...", quality);
        let url = Self::fetch_from_ytdlp(yt_dlp_path, video_url, quality).await?;
        
        // Log the newly fetched URL with hash
        let hash: u64 = url.bytes().fold(0u64, |acc, b| acc.wrapping_add(b as u64));
        println!("[UrlCache] Fetched {} - hash:{} url:{}...", quality, hash, &url[..60.min(url.len())]);
        
        // Store in cache
        self.set(video_url, quality, url.clone()).await;
        
        Ok(url)
    }

    /// Fetch streaming URL from yt-dlp
    async fn fetch_from_ytdlp(
        yt_dlp_path: &std::path::Path,
        video_url: &str,
        quality: &str,
    ) -> Result<String, String> {
        // CRITICAL FIX: YouTube progressive formats (18, 22) don't exist for most videos anymore!
        // YouTube now serves:
        //   - Format 18 = 360p combined (video+audio) - OFTEN THE ONLY PROGRESSIVE FORMAT
        //   - Other resolutions are video-only (no audio) in progressive
        //   - HLS formats (91-96) have audio at all quality levels
        // 
        // HLS Format IDs with audio:
        //   91 = 144p, 92 = 240p, 93 = 360p, 94 = 480p, 95 = 720p, 96 = 1080p
        //
        // We'll use HLS formats as primary, with progressive fallback
        let format_selector = match quality {
            "360p" => "93/18/best[height<=360][acodec!=none]",
            "480p" => "94/best[height<=480][acodec!=none]",
            "720p" => "95/22/best[height<=720][acodec!=none]",
            "1080p" => "96/best[height<=1080][acodec!=none]",
            _ => "best[acodec!=none]",
        };
        
        println!("[UrlCache] Fetching {} with format: {}", quality, format_selector);

        let mut cmd = Command::new(yt_dlp_path);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);

        cmd.arg(video_url)
            .arg("-g")
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
            return Err(format!("Failed to get {} URL: {}", quality, stderr));
        }

        let url_output = String::from_utf8_lossy(&output.stdout);
        let url = url_output
            .lines()
            .next()
            .ok_or("No streaming URL returned")?
            .trim()
            .to_string();

        if url.is_empty() {
            return Err(format!("Empty URL returned for {}", quality));
        }

        println!("[UrlCache] Fetched {} URL: {}...", quality, &url[..60.min(url.len())]);
        Ok(url)
    }

    /// Clear all cached URLs (useful when switching videos)
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Clear cached URLs for a specific video
    pub async fn clear_video(&self, video_url: &str) {
        let mut cache = self.cache.write().await;
        cache.retain(|key, _| !key.starts_with(video_url));
    }
}
