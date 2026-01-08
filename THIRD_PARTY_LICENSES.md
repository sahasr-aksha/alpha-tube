<p align="center">
  <img src="./readme-banner.png" alt="Alpha Tube" width="100%">
</p>

# 📜 Third-Party Licenses

Alpha Tube is built upon the following excellent open-source projects. We are deeply grateful to their maintainers and contributors.

---

## 🔧 Bundled Binaries

### FFmpeg (Custom Build)

| | |
|---|---|
| **Website** | https://ffmpeg.org/ |
| **License** | ![GPL v3](https://img.shields.io/badge/License-GPL%20v3-blue) |
| **Purpose** | Audio/video processing and format conversion |
| **Source** | https://git.ffmpeg.org/ffmpeg.git |

Custom minimal build compiled from source with the following configuration:

```
--disable-everything --disable-debug --disable-doc --disable-ffplay 
--disable-network --enable-small --enable-gpl --enable-version3 
--enable-ffmpeg --enable-ffprobe --enable-libmp3lame 
--enable-muxer=mp4,hls,mpegts,mp3,mov,adts,segment 
--enable-demuxer=mov,matroska,flv,avi,mp3,aac,hls 
--enable-decoder=h264,hevc,vp9,vp8,av1,aac,mp3,mp3float,opus,vorbis,ac3,eac3,flac 
--enable-encoder=libmp3lame,aac 
--enable-parser=h264,hevc,aac,vp9,av1,opus,vorbis,mpegaudio 
--enable-protocol=file,pipe 
--enable-bsf=aac_adtstoasc,h264_mp4toannexb,hevc_mp4toannexb,vp9_superframe,extract_extradata
```

> 📁 See `custom-ffmpeg/BUILD_REPORT.md` for complete build details.

---

### yt-dlp

| | |
|---|---|
| **Website** | https://github.com/yt-dlp/yt-dlp |
| **License** | ![Unlicense](https://img.shields.io/badge/License-Unlicense-brightgreen) |
| **Purpose** | Multi-platform video downloading |

---

## 🎨 Frontend Dependencies

| Package | License | Purpose |
|---------|---------|---------|
| [React](https://react.dev/) | ![MIT](https://img.shields.io/badge/License-MIT-green) | UI Framework |
| [Vidstack](https://vidstack.io/) | ![MIT](https://img.shields.io/badge/License-MIT-green) | Video Player |
| [Framer Motion](https://www.framer.com/motion/) | ![MIT](https://img.shields.io/badge/License-MIT-green) | Animations |
| [Lucide Icons](https://lucide.dev/) | ![ISC](https://img.shields.io/badge/License-ISC-blue) | Icon Set |

---

## ⚙️ Backend Dependencies

| Package | License | Purpose |
|---------|---------|---------|
| [Tauri](https://tauri.app/) | ![MIT/Apache-2.0](https://img.shields.io/badge/License-MIT%20%7C%20Apache--2.0-blue) | Desktop Framework |
| [Rust](https://www.rust-lang.org/) | ![MIT/Apache-2.0](https://img.shields.io/badge/License-MIT%20%7C%20Apache--2.0-blue) | Backend Language |

---

## 🛠️ Build Tools

| Package | License | Purpose |
|---------|---------|---------|
| [Vite](https://vitejs.dev/) | ![MIT](https://img.shields.io/badge/License-MIT-green) | Build Tool |
| [TypeScript](https://www.typescriptlang.org/) | ![Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue) | Type System |
| [Tailwind CSS](https://tailwindcss.com/) | ![MIT](https://img.shields.io/badge/License-MIT-green) | Utility CSS |

---

## 📄 GPL Compliance Notice

FFmpeg is licensed under the **GNU General Public License v3 (GPL v3)**. In compliance with this license:

| Requirement | Fulfilled |
|-------------|-----------|
| Source code availability | ✅ https://git.ffmpeg.org/ffmpeg.git |
| Build configuration | ✅ See `custom-ffmpeg/BUILD_REPORT.md` |
| License text | ✅ https://www.gnu.org/licenses/gpl-3.0.html |
| Attribution | ✅ This document |

---

<p align="center">
  <sub>Last updated: January 2026</sub>
</p>
