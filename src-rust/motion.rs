use std::{collections::BTreeMap, fs, path::Path};

use serde::Deserialize;

#[derive(Default)]
pub struct MotionLibrary {
    motions: BTreeMap<String, Vec<i32>>,
    _loop_counts: BTreeMap<String, u32>,
}

#[derive(Deserialize)]
struct MotionDocument {
    #[serde(default)]
    motions: BTreeMap<String, MotionDefinition>,
}

#[derive(Deserialize)]
struct MotionDefinition {
    frames: Vec<i32>,
    #[serde(default, rename = "loop")]
    loop_count: u32,
}

impl MotionLibrary {
    pub fn load(actions_dir: &Path) -> Self {
        let mut library = Self::default();
        let mut pack_dirs = match fs::read_dir(actions_dir) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    entry
                        .file_type()
                        .ok()
                        .filter(|file_type| file_type.is_dir())
                        .map(|_| entry.path())
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                eprintln!("无法读取动作包目录 {}：{error}", actions_dir.display());
                return library;
            }
        };
        pack_dirs.sort();

        for pack_dir in pack_dirs {
            let motions_path = pack_dir.join("motions.json");
            let manifest_path = pack_dir.join("manifest.json");
            let source = if motions_path.is_file() {
                Some(motions_path)
            } else if manifest_path.is_file() {
                Some(manifest_path)
            } else {
                None
            };
            let Some(source) = source else { continue };
            let document = match load_document(&source) {
                Ok(document) => document,
                Err(error) => {
                    eprintln!("动作包 {} 解析失败：{error}", pack_dir.display());
                    continue;
                }
            };
            for (scene, definition) in document.motions {
                if definition.frames.is_empty() || definition.frames.iter().any(|frame| *frame < 0)
                {
                    eprintln!(
                        "动作包 {} 的动作 {scene} 包含无效帧，已跳过",
                        pack_dir.display()
                    );
                    continue;
                }
                library.motions.insert(scene.clone(), definition.frames);
                library._loop_counts.insert(scene, definition.loop_count);
            }
        }

        library
    }

    pub fn resolve(&self, scene: &str) -> Vec<i32> {
        self.motions
            .get(scene)
            .cloned()
            .unwrap_or_else(|| default_frames(scene))
    }
}

fn load_document(path: &Path) -> Result<MotionDocument, serde_json::Error> {
    let bytes = fs::read(path).map_err(serde_json::Error::io)?;
    serde_json::from_slice(&bytes)
}

fn default_frames(scene: &str) -> Vec<i32> {
    match scene {
        "bubble" => vec![4, 5, 3],
        "encourage" => vec![6, 6, 6, 3],
        "exit" => vec![4, 5, 3],
        "idle" => vec![0],
        "walk" => vec![0, 2],
        "focus" => vec![1],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        process,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    static NEXT_TEMP_DIR: AtomicUsize = AtomicUsize::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let unique = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "offline-companion-motion-{name}-{}-{unique}",
                process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn add_pack(&self, name: &str, contents: &str) {
            let pack = self.0.join(name);
            fs::create_dir_all(&pack).unwrap();
            fs::write(pack.join("motions.json"), contents).unwrap();
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn resolves_built_in_defaults_without_installed_packs() {
        let actions = TestDir::new("defaults");
        let library = MotionLibrary::load(&actions.0);

        assert_eq!(library.resolve("bubble"), vec![4, 5, 3]);
        assert_eq!(library.resolve("encourage"), vec![6, 6, 6, 3]);
        assert_eq!(library.resolve("exit"), vec![4, 5, 3]);
        assert_eq!(library.resolve("idle"), vec![0]);
        assert_eq!(library.resolve("walk"), vec![0, 2]);
        assert_eq!(library.resolve("focus"), vec![1]);
    }

    #[test]
    fn pack_motion_overrides_built_in_scene() {
        let actions = TestDir::new("override");
        actions.add_pack(
            "custom",
            r#"{"motions":{"bubble":{"frames":[8,9,10],"loop":1}}}"#,
        );

        let library = MotionLibrary::load(&actions.0);

        assert_eq!(library.resolve("bubble"), vec![8, 9, 10]);
    }

    #[test]
    fn partial_override_keeps_other_built_in_defaults() {
        let actions = TestDir::new("partial");
        actions.add_pack(
            "custom",
            r#"{"motions":{"bubble":{"frames":[12],"loop":1}}}"#,
        );

        let library = MotionLibrary::load(&actions.0);

        assert_eq!(library.resolve("bubble"), vec![12]);
        assert_eq!(library.resolve("encourage"), vec![6, 6, 6, 3]);
    }

    #[test]
    fn malformed_file_is_skipped_without_losing_defaults() {
        let actions = TestDir::new("malformed");
        actions.add_pack("broken", r#"{"motions":{"bubble":"#);

        let library = MotionLibrary::load(&actions.0);

        assert_eq!(library.resolve("bubble"), vec![4, 5, 3]);
    }

    #[test]
    fn unknown_scene_returns_empty_sequence() {
        let actions = TestDir::new("unknown");
        let library = MotionLibrary::load(&actions.0);

        assert!(library.resolve("not-a-scene").is_empty());
    }

    #[test]
    fn extra_unknown_keys_are_ignored() {
        let actions = TestDir::new("extra-keys");
        actions.add_pack(
            "custom",
            r#"{
                "formatVersion": 2,
                "motions": {
                    "encourage": {
                        "frames": [11, 3],
                        "loop": 1,
                        "sound": "cheer.wav"
                    }
                }
            }"#,
        );

        let library = MotionLibrary::load(&actions.0);

        assert_eq!(library.resolve("encourage"), vec![11, 3]);
    }

    #[test]
    fn loop_field_is_optional() {
        let actions = TestDir::new("optional-loop");
        actions.add_pack("custom", r#"{"motions":{"walk":{"frames":[2,0,2]}}}"#);

        let library = MotionLibrary::load(&actions.0);

        assert_eq!(library.resolve("walk"), vec![2, 0, 2]);
    }
}
