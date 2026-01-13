//! Progress parsing for yt-dlp output
//!
//! Parses stdout lines to extract download progress information.
//! Based on proven patterns from Fall-X repository.

#![allow(dead_code)]

use regex::Regex;
use std::sync::LazyLock;

/// Regex for yt-dlp progress format:
/// [download]  45.2% of 150.32MiB at 2.50MiB/s ETA 00:35
static YTDLP_PROGRESS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\[download\]\s+(\d+\.?\d*)%\s+of\s+~?(\d+\.?\d*)(Ki?B|Mi?B|Gi?B)\s+at\s+(\d+\.?\d*)(Ki?B|Mi?B|Gi?B)/s(?:.*ETA\s+(\d+):(\d+))?"
    ).unwrap()
});

/// Regex for muxing/merging detection
static MUXING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[Merger\]|\[ffmpeg\]|Merging").unwrap()
});

/// Regex for completed download
static COMPLETED_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[download\] 100%|\[download\].*has already been downloaded").unwrap()
});

/// Regex for destination filename
static DESTINATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[download\] Destination:\s*(.+)").unwrap()
});

/// Regex to strip ANSI escape codes
static ANSI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1B\[[0-9;]*[a-zA-Z]").unwrap()
});

/// Progress update types
#[derive(Debug, Clone)]
pub enum ProgressUpdate {
    /// Download progress with percent, speed, ETA
    Progress {
        percent: f64,
        speed: String,
        eta: String,
    },
    /// Muxing/merging phase detected
    Muxing,
    /// Download completed
    Completed,
    /// Destination filename detected
    Destination(String),
}

/// Strip ANSI escape codes from a string
pub fn strip_ansi(s: &str) -> String {
    ANSI_RE.replace_all(s, "").to_string()
}

/// Parse a line of yt-dlp output for progress info
pub fn parse_progress_line(line: &str) -> Option<ProgressUpdate> {
    // Strip ANSI codes first
    let clean_line = strip_ansi(line);

    // Check for muxing/merging
    if MUXING_RE.is_match(&clean_line) {
        return Some(ProgressUpdate::Muxing);
    }

    // Check for completion
    if COMPLETED_RE.is_match(&clean_line) {
        return Some(ProgressUpdate::Completed);
    }

    // Check for destination filename
    if let Some(caps) = DESTINATION_RE.captures(&clean_line) {
        if let Some(filename) = caps.get(1) {
            return Some(ProgressUpdate::Destination(filename.as_str().trim().to_string()));
        }
    }

    // Try to parse progress
    if let Some(caps) = YTDLP_PROGRESS_RE.captures(&clean_line) {
        let percent: f64 = caps.get(1)?.as_str().parse().ok()?;
        let speed_val: f64 = caps.get(4)?.as_str().parse().ok()?;
        let speed_unit = caps.get(5)?.as_str();
        let speed = format!("{:.2}{}/s", speed_val, speed_unit);

        // Optional ETA
        let eta = if let (Some(m_min), Some(m_sec)) = (caps.get(6), caps.get(7)) {
            format!("{}:{}", m_min.as_str(), m_sec.as_str())
        } else {
            String::new()
        };

        return Some(ProgressUpdate::Progress { percent, speed, eta });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ytdlp_progress() {
        let line = "[download]  45.2% of 150.32MiB at 2.50MiB/s ETA 00:35";
        let result = parse_progress_line(line);
        assert!(result.is_some());
        if let Some(ProgressUpdate::Progress { percent, .. }) = result {
            assert!((percent - 45.2).abs() < 0.1);
        }
    }

    #[test]
    fn test_muxing_detection() {
        let line = "[Merger] Merging formats into \"output.mp4\"";
        let result = parse_progress_line(line);
        assert!(matches!(result, Some(ProgressUpdate::Muxing)));

        let line2 = "[ffmpeg] Merging formats";
        let result2 = parse_progress_line(line2);
        assert!(matches!(result2, Some(ProgressUpdate::Muxing)));
    }

    #[test]
    fn test_destination_detection() {
        let line = "[download] Destination: /path/to/video.mp4";
        let result = parse_progress_line(line);
        if let Some(ProgressUpdate::Destination(filename)) = result {
            assert_eq!(filename, "/path/to/video.mp4");
        } else {
            panic!("Expected Destination variant");
        }
    }

    #[test]
    fn test_ansi_stripping() {
        let line = "\x1B[32m[download]\x1B[0m  45.2% of 150.32MiB at 2.50MiB/s ETA 00:35";
        let result = parse_progress_line(line);
        assert!(matches!(result, Some(ProgressUpdate::Progress { .. })));
    }

    #[test]
    fn test_completed_detection() {
        let line = "[download] 100% of 150.32MiB";
        let result = parse_progress_line(line);
        assert!(matches!(result, Some(ProgressUpdate::Completed)));
    }
}
