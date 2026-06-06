use reqwest::{Client, Proxy};
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tauri::AppHandle;
use directories::ProjectDirs;

// 下载对应平台的 Syncthing 核心
pub async fn download_syncthing(bin_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");

    if let Ok(proxy_url) = env::var("HTTPS_PROXY").or_else(|_| env::var("https_proxy")) {
        println!("检测到代理：{}，将使用代理进行下载", proxy_url);
        builder = builder.proxy(Proxy::all(proxy_url)?);
    } else {
        // Fallback for current local port
        println!("尝试默认走 127.0.0.1:10808 代理...");
        builder = builder.proxy(Proxy::all("http://127.0.0.1:10808")?);
    }

    let client = builder.build()?;

    let (url, filename) = get_download_info();

    println!("开始下载 Syncthing: {}", url);
    let mut response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(format!("下载失败: {}", response.status()).into());
    }

    let download_dir = bin_path.parent().unwrap();
    if !download_dir.exists() {
        fs::create_dir_all(download_dir)?;
    }

    let archive_path = download_dir.join(filename);
    let mut file = File::create(&archive_path)?;

    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk)?;
    }

    println!("下载完成，准备解压...");
    extract_syncthing(&archive_path, download_dir)?;

    // 赋予执行权限 (Linux/macOS)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(bin_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(bin_path, perms)?;
    }

    Ok(())
}

fn get_download_info() -> (String, String) {
    let version = "v1.27.7"; // 这里可以写死一个较新的稳定版本，或动态获取 GitHub Latest API

    #[cfg(target_os = "windows")]
    let (arch, ext) = match env::consts::ARCH {
        "x86_64" => ("windows-amd64", "zip"),
        "aarch64" => ("windows-arm64", "zip"),
        _ => ("windows-amd64", "zip"), // 默认 fallback
    };

    #[cfg(target_os = "linux")]
    let (arch, ext) = match env::consts::ARCH {
        "x86_64" => ("linux-amd64", "tar.gz"),
        "aarch64" => ("linux-arm64", "tar.gz"),
        _ => ("linux-amd64", "tar.gz"),
    };

    #[cfg(target_os = "macos")]
    let (arch, ext) = match env::consts::ARCH {
        "aarch64" => ("macos-arm64", "zip"),
        "x86_64" => ("macos-amd64", "zip"),
        _ => ("macos-amd64", "zip"),
    };

    let filename = format!("syncthing-{}-{}.{}", arch, version, ext);
    let url = format!("https://github.com/syncthing/syncthing/releases/download/{}/{}", version, filename);

    (url, filename)
}

fn extract_syncthing(archive_path: &PathBuf, target_dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let ext = archive_path.extension().unwrap().to_str().unwrap();
    let file = File::open(archive_path)?;

    if ext == "zip" {
        let mut archive = zip::ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = match file.enclosed_name() {
                Some(path) => target_dir.join(path),
                None => continue,
            };

            if (*file.name()).ends_with('/') {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        fs::create_dir_all(p)?;
                    }
                }

                // 仅提取核心的二进制文件，扁平化放到 target 目录下
                let file_name = outpath.file_name().unwrap().to_str().unwrap();
                if file_name == "syncthing" || file_name == "syncthing.exe" {
                    let final_bin_path = target_dir.join(file_name);
                    let mut outfile = File::create(&final_bin_path)?;
                    std::io::copy(&mut file, &mut outfile)?;
                }
            }
        }
    } else {
        // tar.gz
        let tar = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(tar);

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.to_path_buf(); // 将 path 转换为 PathBuf 以进行所有权分离
            let file_name = path.file_name().unwrap_or_default().to_str().unwrap_or_default();

            // 我们只需要那个真正的可执行文件，忽略 etc 或者 FreeBSD 等子目录里的脚本
            if file_name == "syncthing" || file_name == "syncthing.exe" {
                if !path.components().any(|c| c.as_os_str() == "etc") {
                    let final_bin_path = target_dir.join(file_name);
                    entry.unpack(&final_bin_path)?;
                }
            }
        }
    }

    // 清理压缩包
    fs::remove_file(archive_path)?;

    Ok(())
}

pub fn get_app_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "AeroSync", "AeroSyncApp") {
        proj_dirs.data_dir().to_path_buf()
    } else {
        // Fallback 到当前目录
        PathBuf::from(".")
    }
}
