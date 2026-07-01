# YT Downloader

A lightweight Windows desktop app to download YouTube videos and audio using yt-dlp.

## Features

- Paste a YouTube URL and see available resolutions
- Download as MP4 (up to 4K) or extract audio only
- Shows real-time download progress with speed and ETA
- Resizable output window with video preview
- Supports Shorts (vertical videos)

## Download

[**Download latest installer (MSI)**](https://github.com/Manas-Kushwaha-99/yt-downloader/releases/latest)

## Build from source

### Prerequisites

- Node.js 18+
- Rust toolchain
- Windows SDK (for icon embedding)

### Steps

```bash
git clone https://github.com/Manas-Kushwaha-99/yt-downloader.git
cd yt-downloader
npm install
cd src-tauri && cargo build --release && cd ..
npx tauri build
```

The installer will be at `src-tauri/target/release/bundle/msi/`.

## Tech Stack

- **Tauri v2** — desktop framework
- **yt-dlp** — video/audio extraction
- **Vanilla HTML/CSS/JS** — frontend
