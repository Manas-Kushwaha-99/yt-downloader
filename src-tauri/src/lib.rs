use std::path::PathBuf;
use std::process::Stdio;
use tauri::{AppHandle, Emitter, Manager};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};

const NO_WINDOW: u32 = 0x08000000;

#[derive(Clone, Serialize)]
struct DownloadProgress {
    percent: f64,
    total_size: String,
    eta: String,
    speed: String,
    filename: String,
}

#[derive(Clone, Serialize)]
struct UpdateLine {
    line: String,
}

#[derive(Clone, Serialize)]
struct DownloadComplete {
    filename: String,
    file_size: String,
}

#[derive(Clone, Serialize)]
struct DownloadErrorPayload {
    error_type: String,
    message: String,
}

#[derive(Clone, Serialize)]
struct FormatsResponse {
    title: String,
    max_quality: u32,
    available_qualities: Vec<u32>,
    has_audio: bool,
    is_vertical: bool,
}

#[derive(Deserialize)]
struct YtDlpFormat {
    height: Option<f64>,
    width: Option<f64>,
    acodec: Option<String>,
}

#[derive(Deserialize)]
struct YtDlpInfo {
    title: String,
    formats: Vec<YtDlpFormat>,
}

fn ytdlp_path(app: &AppHandle) -> PathBuf {
    let target_triple = option_env!("TAURI_ENV_TARGET_TRIPLE")
        .unwrap_or("x86_64-pc-windows-msvc");
    let exe_name = format!("yt-dlp-{}.exe", target_triple);

    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled = resource_dir.join("binaries").join(&exe_name);
        if bundled.exists() {
            return bundled;
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_path = manifest_dir.join("binaries").join(&exe_name);
    if dev_path.exists() {
        return dev_path;
    }

    PathBuf::from("yt-dlp.exe")
}

fn ytdlp_cmd(app: &AppHandle) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(ytdlp_path(app));
    #[cfg(windows)]
    cmd.creation_flags(NO_WINDOW);
    cmd
}

#[tauri::command]
async fn check_ytdlp(app: AppHandle) -> Result<String, String> {
    let output = ytdlp_cmd(&app)
        .arg("--version")
        .output()
        .await
        .map_err(|e| format!("yt-dlp not found: {}", e))?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(version)
    } else {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("yt-dlp error: {}", err))
    }
}

#[tauri::command]
async fn update_ytdlp(app: AppHandle) -> Result<String, String> {
    let mut child = ytdlp_cmd(&app)
        .arg("-U")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let app_stdout = app.clone();
    let stdout_task = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            app_stdout.emit("update-line", UpdateLine { line: line.clone() }).ok();
        }
    });

    let app_stderr = app.clone();
    let stderr_task = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            app_stderr.emit("update-line", UpdateLine { line: line.clone() }).ok();
        }
    });

    stdout_task.await.ok();
    stderr_task.await.ok();

    let status = child.wait().await.map_err(|e| format!("{}", e))?;

    if status.success() {
        let v = check_ytdlp(app.clone()).await.unwrap_or_else(|_| "unknown".to_string());
        Ok(format!("Updated to {}", v))
    } else {
        Err("Update failed. Check your internet connection.".to_string())
    }
}

#[tauri::command]
async fn fetch_formats(app: AppHandle, url: String) -> Result<FormatsResponse, String> {
    let output = ytdlp_cmd(&app)
        .arg("-j")
        .arg("--no-playlist")
        .arg(&url)
        .output()
        .await
        .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let info: YtDlpInfo =
            serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse video info: {}", e))?;

        let mut qualities: Vec<u32> = info
            .formats
            .iter()
            .filter_map(|f| {
                match (f.width, f.height) {
                    (Some(w), Some(h)) if w > 0.0 && h > 0.0 => {
                        let q = (w.min(h)) as u32;
                        let skip: &[u32] = &[25, 27, 33, 45, 50, 67, 90, 101, 135, 180, 608];
                        if q >= 144 && !skip.contains(&q) { Some(q) } else { None }
                    }
                    _ => None,
                }
            })
            .collect();
        qualities.sort_unstable();
        qualities.dedup();
        qualities.reverse();

        let any_vertical = info.formats.iter().any(|f| {
            match (f.width, f.height) {
                (Some(w), Some(h)) => h > w && w > 100.0,
                _ => false,
            }
        });

        let max_quality = qualities.first().copied().unwrap_or(0);
        let has_audio = info
            .formats
            .iter()
            .any(|f| f.acodec.as_deref() != Some("none") && f.acodec.is_some());

        Ok(FormatsResponse {
            title: info.title,
            max_quality,
            available_qualities: qualities,
            has_audio,
            is_vertical: any_vertical,
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let (_, message) = categorize_error(&stderr);
        Err(message)
    }
}

fn categorize_error(stderr: &str) -> (String, String) {
    let lower = stderr.to_lowercase();
    if lower.contains("age") && (lower.contains("restrict") || lower.contains("limit") || lower.contains("verification")) {
        ("restricted".to_string(), "This video is age-restricted. Unable to download.".to_string())
    } else if lower.contains("private") && lower.contains("video") {
        ("private".to_string(), "This video is private.".to_string())
    } else if lower.contains("video unavailable")
        || lower.contains("deleted")
        || lower.contains("removed")
        || lower.contains("no longer available")
    {
        ("deleted".to_string(), "This video has been deleted or is unavailable.".to_string())
    } else if lower.contains("members")
        && (lower.contains("only") || lower.contains("premium") || lower.contains("paid") || lower.contains("content"))
    {
        ("restricted".to_string(), "This video is restricted (members-only or paid).".to_string())
    } else if lower.contains("sign in")
        || lower.contains("login")
        || lower.contains("authentication")
    {
        ("restricted".to_string(), "This video requires sign-in. Unable to download.".to_string())
    } else {
        ("error".to_string(), "Error. Try again later.".to_string())
    }
}

#[tauri::command]
async fn start_download(
    app: AppHandle,
    url: String,
    quality: String,
    output_dir: String,
    audio_only: bool,
    is_vertical: bool,
) -> Result<String, String> {
    let mut cmd = ytdlp_cmd(&app);
    cmd.arg("--newline");
    cmd.arg("--no-playlist");
    cmd.arg("--no-warnings");

    if audio_only {
        cmd.arg("-f").arg("bestaudio");
        cmd.arg("-o").arg(format!("{}/%(title)s.%(ext)s", output_dir));
    } else if quality == "best" {
        cmd.arg("-f").arg("bestvideo+bestaudio/best");
        cmd.arg("--merge-output-format").arg("mp4");
        cmd.arg("-o").arg(format!("{}/%(title)s.%(ext)s", output_dir));
    } else {
        let dim = if is_vertical { "width" } else { "height" };
        let fmt = format!(
            "bestvideo[{}<={}]+bestaudio/best[{}<={}]",
            dim, quality, dim, quality
        );
        cmd.arg("-f").arg(&fmt);
        cmd.arg("--merge-output-format").arg("mp4");
        cmd.arg("-o").arg(format!("{}/%(title)s.%(ext)s", output_dir));
    }

    cmd.arg(&url);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to start download: {}", e))?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let app_progress = app.clone();
    let progress_with_eta = Regex::new(
        r"^\[download\]\s+(\d+\.?\d*)%\s+of\s+~?(.+?)\s+at\s+(.+?)\s+ETA\s+(\S+)"
    ).unwrap();
    let progress_done = Regex::new(
        r"^\[download\]\s+(\d+)%\s+of\s+~?(.+?)\s+in\s+(\S+)"
    ).unwrap();
    let dest_re = Regex::new(r"^\[download\]\s+Destination:\s+(.+)").unwrap();

    let _reader_task = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut current_filename = String::new();

        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(caps) = dest_re.captures(&line) {
                current_filename = caps.get(1).unwrap().as_str().trim().to_string();
                app_progress
                    .emit("download-progress", DownloadProgress {
                        percent: 0.0,
                        total_size: String::new(),
                        eta: String::new(),
                        speed: String::new(),
                        filename: current_filename.clone(),
                    })
                    .ok();
                continue;
            }

            if let Some(caps) = progress_with_eta.captures(&line) {
                let percent: f64 = caps.get(1).unwrap().as_str().parse().unwrap_or(0.0);
                let total_size = caps.get(2).unwrap().as_str().trim().to_string();
                let speed = caps.get(3).unwrap().as_str().trim().to_string();
                let eta = caps.get(4).unwrap().as_str().trim().to_string();

                app_progress
                    .emit("download-progress", DownloadProgress {
                        percent,
                        total_size,
                        eta,
                        speed,
                        filename: current_filename.clone(),
                    })
                    .ok();
                continue;
            }

            if let Some(caps) = progress_done.captures(&line) {
                caps.get(1).unwrap().as_str().parse::<f64>().unwrap_or(100.0);
                let total_size = caps.get(2).unwrap().as_str().trim().to_string();
                let eta = caps.get(3).unwrap().as_str().trim().to_string();

                app_progress
                    .emit("download-progress", DownloadProgress {
                        percent: 100.0,
                        total_size,
                        eta,
                        speed: "Done".to_string(),
                        filename: current_filename.clone(),
                    })
                    .ok();
            }
        }
    });

    let stderr_task = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        let mut all_stderr = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            all_stderr.push_str(&line);
            all_stderr.push('\n');
        }
        all_stderr
    });

    let status = child.wait().await.map_err(|e| format!("Download process error: {}", e))?;
    let all_stderr = stderr_task.await.unwrap_or_default();

    if status.success() {
        app.emit("download-complete", DownloadComplete {
            filename: "Download finished".to_string(),
            file_size: String::new(),
        }).ok();
        Ok("Download complete".to_string())
    } else {
        let (error_type, message) = categorize_error(&all_stderr);
        app.emit("download-error", DownloadErrorPayload {
            error_type: error_type.clone(),
            message: message.clone(),
        }).ok();
        Err(message)
    }
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;
    let result = app.dialog().file().blocking_pick_folder();
    match result {
        Some(path) => Ok(path.to_string()),
        None => Ok(String::new()),
    }
}

#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("Failed to open folder: {}", e))?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            check_ytdlp,
            update_ytdlp,
            fetch_formats,
            start_download,
            pick_folder,
            open_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
