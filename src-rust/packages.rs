use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Deserialize;
use zip::ZipArchive;

const MAX_PACKAGE_BYTES: u64 = 200 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;
const ALLOWED_EXTENSIONS: &[&str] = &["json", "png", "webp", "wav", "ogg", "txt"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    package_type: String,
    id: String,
    version: String,
}

pub fn install_package(zip_path: &Path, characters: &Path, actions: &Path) -> Result<String> {
    let file = fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file).context("不是有效的 ZIP 扩展包")?;
    let mut total = 0u64;
    let mut manifest_text = None;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let path = safe_relative_path(entry.name())?;
        if entry.is_dir() {
            continue;
        }
        anyhow::ensure!(
            entry.size() <= MAX_FILE_BYTES,
            "扩展包内单个文件超过 100 MB"
        );
        total = total.saturating_add(entry.size());
        anyhow::ensure!(total <= MAX_PACKAGE_BYTES, "扩展包解压总量超过 200 MB");
        let extension = path
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        anyhow::ensure!(
            ALLOWED_EXTENSIONS.contains(&extension.as_str()),
            "扩展包包含不允许的文件类型：{}",
            extension
        );
        if path == Path::new("manifest.json") {
            let mut text = String::new();
            io::Read::read_to_string(&mut entry, &mut text)?;
            manifest_text = Some(text);
        }
    }

    let manifest: Manifest =
        serde_json::from_str(&manifest_text.context("扩展包缺少 manifest.json")?)?;
    anyhow::ensure!(manifest.schema_version == 1, "不支持的 Manifest 版本");
    semver::Version::parse(&manifest.version).context("扩展包版本号无效")?;
    anyhow::ensure!(
        manifest
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-')),
        "扩展包 ID 无效"
    );
    let base = match manifest.package_type.as_str() {
        "character" => characters,
        "action" => actions,
        _ => anyhow::bail!("未知扩展包类型"),
    };
    let destination = base.join(format!("{}-{}", manifest.id, manifest.version));
    let temporary = base.join(format!(".installing-{}", manifest.id));
    if temporary.exists() {
        fs::remove_dir_all(&temporary)?;
    }
    fs::create_dir_all(&temporary)?;

    let file = fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let relative = safe_relative_path(entry.name())?;
        let target = temporary.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut output = fs::File::create(target)?;
            io::copy(&mut entry, &mut output)?;
        }
    }
    if manifest.package_type == "action" {
        let motions_path = temporary.join("motions.json");
        if motions_path.is_file() {
            let valid = fs::File::open(&motions_path)
                .ok()
                .and_then(|file| serde_json::from_reader::<_, serde_json::Value>(file).ok())
                .is_some();
            if !valid {
                let _ = fs::remove_dir_all(&temporary);
                anyhow::bail!("动作包 motions.json 格式无效");
            }
        }
    }
    if destination.exists() {
        fs::remove_dir_all(&destination)?;
    }
    fs::rename(&temporary, &destination)?;
    Ok(format!("已安装 {} {}", manifest.id, manifest.version))
}

fn safe_relative_path(raw: &str) -> Result<PathBuf> {
    let path = Path::new(raw);
    anyhow::ensure!(!path.is_absolute(), "扩展包包含绝对路径");
    anyhow::ensure!(
        path.components()
            .all(|part| matches!(part, Component::Normal(_))),
        "扩展包包含不安全路径"
    );
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use std::{
        io::Write,
        process,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use zip::{ZipWriter, write::SimpleFileOptions};

    use super::*;

    static NEXT_TEMP_DIR: AtomicUsize = AtomicUsize::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let unique = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "offline-companion-package-{name}-{}-{unique}",
                process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_action_package(path: &Path, id: &str, motions: &str) {
        let file = fs::File::create(path).unwrap();
        let mut archive = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        archive.start_file("manifest.json", options).unwrap();
        write!(
            archive,
            r#"{{"schemaVersion":1,"packageType":"action","id":"{id}","version":"1.0.0"}}"#
        )
        .unwrap();
        archive.start_file("motions.json", options).unwrap();
        archive.write_all(motions.as_bytes()).unwrap();
        archive.finish().unwrap();
    }

    #[test]
    fn rejects_traversal_paths() {
        assert!(safe_relative_path("../escape.txt").is_err());
        assert!(safe_relative_path("/absolute.txt").is_err());
        assert!(safe_relative_path("animations/idle.json").is_ok());
    }

    #[test]
    fn valid_action_motions_file_installs() {
        let root = TestDir::new("valid-motions");
        let characters = root.0.join("characters");
        let actions = root.0.join("actions");
        fs::create_dir_all(&characters).unwrap();
        fs::create_dir_all(&actions).unwrap();
        let package = root.0.join("valid.zip");
        write_action_package(
            &package,
            "valid-motion",
            r#"{"motions":{"bubble":{"frames":[4,5,3],"loop":1}}}"#,
        );

        let status = install_package(&package, &characters, &actions).unwrap();

        assert_eq!(status, "已安装 valid-motion 1.0.0");
        assert!(actions.join("valid-motion-1.0.0/motions.json").is_file());
    }

    #[test]
    fn invalid_action_motions_file_is_rejected() {
        let root = TestDir::new("invalid-motions");
        let characters = root.0.join("characters");
        let actions = root.0.join("actions");
        fs::create_dir_all(&characters).unwrap();
        fs::create_dir_all(&actions).unwrap();
        let package = root.0.join("invalid.zip");
        write_action_package(&package, "invalid-motion", r#"{"motions":"#);

        let error = install_package(&package, &characters, &actions).unwrap_err();

        assert_eq!(error.to_string(), "动作包 motions.json 格式无效");
        assert!(!actions.join("invalid-motion-1.0.0").exists());
        assert!(!actions.join(".installing-invalid-motion").exists());
    }
}
