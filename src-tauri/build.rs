use std::path::PathBuf;

fn main() {
    let target = std::env::var("TARGET").unwrap_or_else(|_| "x86_64-pc-windows-msvc".to_string());
    let exe_name = format!("yt-dlp-{}.exe", target);
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let dest = manifest_dir.join("binaries").join(&exe_name);

    if !dest.exists() {
        std::fs::create_dir_all(dest.parent().unwrap()).ok();

        let url = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
        println!("cargo:warning=Downloading yt-dlp from {} to {}", url, dest.display());

        let status = std::process::Command::new("powershell")
            .args(["-Command", &format!(
                "try {{ Invoke-WebRequest -Uri '{}' -OutFile '{}' -ErrorAction Stop }} catch {{ exit 1 }}",
                url, dest.display()
            )])
            .status();

        match status {
            Ok(s) if s.success() => println!("cargo:warning=yt-dlp downloaded successfully"),
            _ => println!("cargo:warning=Failed to download yt-dlp. It will be resolved from PATH at runtime."),
        }
    }

    // Add RC.EXE to PATH so embed-resource can compile icon.ico into the exe.
    #[cfg(windows)]
    {
        let rc_paths = [
            r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\rc.exe",
            r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\rc.exe",
            r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\rc.exe",
        ];
        for rc in rc_paths.iter() {
            if std::path::Path::new(rc).exists() {
                if let Some(parent) = std::path::Path::new(rc).parent() {
                    let new_path = format!("{};{}",
                        parent.display(),
                        std::env::var("PATH").unwrap_or_default());
                    std::env::set_var("PATH", new_path);
                    println!("cargo:warning=Added RC.EXE to PATH: {}", parent.display());
                }
                break;
            }
        }
    }

    tauri_build::build()
}

