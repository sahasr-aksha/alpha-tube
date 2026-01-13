// Stream proxy module - provides local HTTP server to proxy YouTube streams
// This bypasses CORS/auth issues by fetching with proper headers
// Now supports HLS m3u8 manifests by rewriting segment URLs

use std::sync::Arc;
use tokio::sync::RwLock;
use warp::Filter;
use futures_util::StreamExt;
use base64::{Engine as _, engine::general_purpose};

/// Shared state for the streaming proxy
pub struct StreamProxy {
    pub port: u16,
    pub current_url: Arc<RwLock<Option<String>>>,
    pub headers: Arc<RwLock<Vec<(String, String)>>>,
}

impl StreamProxy {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            current_url: Arc::new(RwLock::new(None)),
            headers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Set the URL to proxy
    pub async fn set_url(&self, url: String, headers: Vec<(String, String)>) {
        println!("[StreamProxy] Setting URL: {}...", &url[..80.min(url.len())]);
        *self.current_url.write().await = Some(url);
        *self.headers.write().await = headers;
    }

    /// Get the local proxy URL that frontend should use
    pub fn get_local_url(&self) -> String {
        format!("http://127.0.0.1:{}/stream", self.port)
    }
}

/// Start the proxy server (call once at app startup)
pub async fn start_proxy_server(proxy: Arc<StreamProxy>) {
    let proxy_clone = proxy.clone();
    let proxy_clone2 = proxy.clone();
    
    // Main stream endpoint - uses current_url from state
    let stream_filter = warp::path("stream")
        .and(warp::any().map(move || proxy_clone.clone()))
        .and(warp::header::optional::<String>("range"))
        .and_then(handle_stream);
    
    // Segment proxy endpoint - proxies arbitrary URLs (for HLS segments)
    let segment_filter = warp::path("segment")
        .and(warp::any().map(move || proxy_clone2.clone()))
        .and(warp::query::<SegmentQuery>())
        .and(warp::header::optional::<String>("range"))
        .and_then(handle_segment);

    let routes = stream_filter.or(segment_filter);

    println!("[StreamProxy] Starting on port {} with HLS support", 9876);
    
    // Run server in background
    tokio::spawn(async move {
        warp::serve(routes)
            .run(([127, 0, 0, 1], 9876))
            .await;
    });
}

#[derive(serde::Deserialize)]
struct SegmentQuery {
    url: String,
}

/// Handle segment requests - proxies any URL passed as query param (base64 encoded)
async fn handle_segment(
    _proxy: Arc<StreamProxy>,
    query: SegmentQuery,
    range_header: Option<String>,
) -> Result<warp::reply::Response, warp::Rejection> {
    // Decode the base64-encoded URL
    let url = match general_purpose::URL_SAFE_NO_PAD.decode(&query.url) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[StreamProxy] Invalid UTF-8 in segment URL: {}", e);
                let response = warp::http::Response::builder()
                    .status(400)
                    .body("Invalid URL encoding".to_string())
                    .unwrap();
                return Ok(warp::reply::Reply::into_response(response));
            }
        },
        Err(e) => {
            eprintln!("[StreamProxy] Failed to decode segment URL: {}", e);
            let response = warp::http::Response::builder()
                .status(400)
                .body("Invalid base64 URL".to_string())
                .unwrap();
            return Ok(warp::reply::Reply::into_response(response));
        }
    };
    
    println!("[StreamProxy] Proxying segment: {}...", &url[..60.min(url.len())]);
    
    proxy_url(&url, range_header).await
}

/// Handle main stream - uses URL from state, handles HLS rewriting
async fn handle_stream(
    proxy: Arc<StreamProxy>,
    range_header: Option<String>,
) -> Result<warp::reply::Response, warp::Rejection> {
    let url = {
        let guard = proxy.current_url.read().await;
        match guard.as_ref() {
            Some(u) => {
                println!("[StreamProxy] Serving URL: {}...", &u[..80.min(u.len())]);
                u.clone()
            },
            None => {
                let response = warp::http::Response::builder()
                    .status(404)
                    .body("No stream URL set".to_string())
                    .unwrap();
                return Ok(warp::reply::Reply::into_response(response));
            }
        }
    };

    // Check if this is an HLS manifest
    if url.contains(".m3u8") || url.contains("manifest") {
        return handle_hls_manifest(&url, proxy.port).await;
    }

    proxy_url(&url, range_header).await
}

/// Handle HLS manifest - fetch, rewrite URLs, return modified
async fn handle_hls_manifest(url: &str, port: u16) -> Result<warp::reply::Response, warp::Rejection> {
    println!("[StreamProxy] Handling HLS manifest: {}...", &url[..60.min(url.len())]);
    
    let client = reqwest::Client::new();
    let response = client.get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .header("Referer", "https://www.youtube.com/")
        .send()
        .await;

    let response = match response {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[StreamProxy] Failed to fetch HLS manifest: {}", e);
            let response = warp::http::Response::builder()
                .status(502)
                .body(format!("Failed to fetch manifest: {}", e))
                .unwrap();
            return Ok(warp::reply::Reply::into_response(response));
        }
    };

    let manifest_text = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[StreamProxy] Failed to read manifest: {}", e);
            let response = warp::http::Response::builder()
                .status(502)
                .body(format!("Failed to read manifest: {}", e))
                .unwrap();
            return Ok(warp::reply::Reply::into_response(response));
        }
    };

    // Rewrite segment URLs to go through our proxy
    let rewritten = rewrite_hls_manifest(&manifest_text, url, port);
    
    println!("[StreamProxy] Rewrote HLS manifest ({} bytes -> {} bytes)", 
        manifest_text.len(), rewritten.len());

    let response = warp::http::Response::builder()
        .status(200)
        .header("Content-Type", "application/vnd.apple.mpegurl")
        .header("Access-Control-Allow-Origin", "*")
        .header("Cache-Control", "no-store, no-cache, must-revalidate, max-age=0")
        .body(rewritten)
        .unwrap();

    Ok(warp::reply::Reply::into_response(response))
}

/// Rewrite HLS manifest URLs to go through our proxy
fn rewrite_hls_manifest(manifest: &str, base_url: &str, port: u16) -> String {
    let mut result = String::new();
    
    // Get base URL for relative paths
    let base = if let Some(idx) = base_url.rfind('/') {
        &base_url[..idx + 1]
    } else {
        base_url
    };

    for line in manifest.lines() {
        if line.starts_with('#') {
            // Comment/directive line - might contain URI
            if line.contains("URI=\"") {
                // Rewrite URI in tags like #EXT-X-KEY
                let rewritten = rewrite_uri_in_tag(line, base, port);
                result.push_str(&rewritten);
            } else {
                result.push_str(line);
            }
        } else if !line.is_empty() {
            // This is a segment URL
            let absolute_url = if line.starts_with("http") {
                line.to_string()
            } else {
                format!("{}{}", base, line)
            };
            
            // Encode the URL using base64 and create proxy URL
            let encoded = general_purpose::URL_SAFE_NO_PAD.encode(&absolute_url);
            let proxy_url = format!("http://127.0.0.1:{}/segment?url={}", port, encoded);
            result.push_str(&proxy_url);
        }
        result.push('\n');
    }

    result
}

/// Rewrite URI in HLS tags like #EXT-X-KEY:METHOD=AES-128,URI="..."
fn rewrite_uri_in_tag(line: &str, base: &str, port: u16) -> String {
    if let Some(start) = line.find("URI=\"") {
        if let Some(end) = line[start + 5..].find('"') {
            let uri = &line[start + 5..start + 5 + end];
            let absolute_url = if uri.starts_with("http") {
                uri.to_string()
            } else {
                format!("{}{}", base, uri)
            };
            let encoded = general_purpose::URL_SAFE_NO_PAD.encode(&absolute_url);
            let proxy_url = format!("http://127.0.0.1:{}/segment?url={}", port, encoded);
            
            return format!("{}URI=\"{}\"{}",
                &line[..start],
                proxy_url,
                &line[start + 5 + end + 1..]
            );
        }
    }
    line.to_string()
}

/// Proxy a URL with proper headers
async fn proxy_url(url: &str, range_header: Option<String>) -> Result<warp::reply::Response, warp::Rejection> {
    let client = reqwest::Client::new();
    let mut req = client.get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .header("Referer", "https://www.youtube.com/");

    // Forward range header if present (for seeking)
    if let Some(range) = &range_header {
        req = req.header("Range", range);
    }

    let response = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[StreamProxy] Request failed: {}", e);
            let response = warp::http::Response::builder()
                .status(502)
                .body(format!("Proxy error: {}", e))
                .unwrap();
            return Ok(warp::reply::Reply::into_response(response));
        }
    };

    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("video/mp4")
        .to_string();
    let content_length = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let content_range = response
        .headers()
        .get("content-range")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Stream the response body
    let stream = response.bytes_stream().map(|result| {
        result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    });

    let body = warp::hyper::Body::wrap_stream(stream);

    // Build response with proper headers
    let mut builder = warp::http::Response::builder()
        .status(status.as_u16())
        .header("Content-Type", content_type)
        .header("Accept-Ranges", "bytes")
        .header("Access-Control-Allow-Origin", "*")
        .header("Cache-Control", "no-store, no-cache, must-revalidate, max-age=0")
        .header("Pragma", "no-cache")
        .header("Expires", "0");

    if let Some(len) = content_length {
        builder = builder.header("Content-Length", len);
    }
    if let Some(range) = content_range {
        builder = builder.header("Content-Range", range);
    }

    Ok(builder.body(body).unwrap())
}
