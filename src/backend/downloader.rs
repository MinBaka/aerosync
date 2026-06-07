use directories::ProjectDirs;
use reqwest::{Client, Proxy};
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub async fn download_syncthing<F>(bin_path: &PathBuf, mut progress_callback: F) -> anyhow::Result<()>
where
    F: FnMut(f32) + Send + 'static,
{
    let mut builder =
        Client::builder().user_agent("AeroSync/0.1.0 (+https://github.com/syncthing/syncthing)");

    if let Ok(proxy_url) = env::var("HTTPS_PROXY").or_else(|_| env::var("https_proxy")) {
        println!("检测到代理：{}，将使用代理进行下载", proxy_url);
        builder = builder.proxy(Proxy::all(proxy_url)?);
    }

    let client = builder.build()?;
    let (url, filename) = get_download_info();

    println!("开始下载 Syncthing: {}", url);
    let mut response = client.get(&url).send().await?;

    if !response.status().is_success() {
        anyhow::bail!("下载失败: {}", response.status());
    }

    let download_dir = bin_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("无效的 Syncthing 二进制路径"))?;
    fs::create_dir_all(download_dir)?;

    let archive_path = download_dir.join(filename);
    let mut file = File::create(&archive_path)?;

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        if total_size > 0 {
            progress_callback(downloaded as f32 / total_size as f32);
        } else {
            progress_callback(0.5); // Indeterminate
        }
    }

    println!("下载完成，准备解压...");
    extract_syncthing(&archive_path, download_dir)?;

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
    let version = "v1.27.7";

    #[cfg(target_os = "windows")]
    let (arch, ext) = match env::consts::ARCH {
        "x86_64" => ("windows-amd64", "zip"),
        "aarch64" => ("windows-arm64", "zip"),
        _ => ("windows-amd64", "zip"),
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
    let url = format!(
        "https://github.com/syncthing/syncthing/releases/download/{}/{}",
        version, filename
    );

    (url, filename)
}

fn extract_syncthing(archive_path: &PathBuf, target_dir: &Path) -> anyhow::Result<()> {
    let file = File::open(archive_path)?;

    if archive_path.to_string_lossy().ends_with(".zip") {
        let mut archive = zip::ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let outpath = match entry.enclosed_name() {
                Some(path) => target_dir.join(path),
                None => continue,
            };

            let file_name = outpath
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if file_name == "syncthing" || file_name == "syncthing.exe" {
                let final_bin_path = target_dir.join(file_name);
                let mut outfile = File::create(&final_bin_path)?;
                std::io::copy(&mut entry, &mut outfile)?;
            }
        }
    } else {
        let tar = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(tar);

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.to_path_buf();
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();

            if (file_name == "syncthing" || file_name == "syncthing.exe")
                && !path
                    .components()
                    .any(|component| component.as_os_str() == "etc")
            {
                entry.unpack(target_dir.join(file_name))?;
            }
        }
    }

    fs::remove_file(archive_path)?;
    Ok(())
}

pub fn get_app_dir() -> PathBuf {
    ProjectDirs::from("com", "AeroSync", "AeroSyncApp")
        .map(|project_dirs| project_dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}
