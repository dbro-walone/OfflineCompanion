use super::{
    manifest::{
        ActionPackEntry, ActionPackManifest, CharacterManifest, PackageManifest, RenderInfo,
    },
    validator::{
        load_manifest, png_dimensions, validate_action_pack_with_legacy, validate_character,
    },
};
use crate::animation::definition::AnimationDefinition;
use semver::Version;
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct CharacterPackage {
    pub root: PathBuf,
    pub manifest: CharacterManifest,
}

#[derive(Debug, Clone)]
pub struct ActionPackage {
    pub root: PathBuf,
    pub manifest: ActionPackManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderSource {
    pub atlas_path: PathBuf,
    pub frame_width: u32,
    pub frame_height: u32,
    pub columns: u32,
    pub rows: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAction {
    pub id: String,
    pub semantic: String,
    pub trigger: String,
    pub weight: u32,
    pub cooldown_ms: u64,
    pub priority: u8,
    pub animation_path: PathBuf,
    pub render: RenderSource,
}

#[derive(Debug, Clone, Default)]
pub struct PackageCatalog {
    pub characters: BTreeMap<String, CharacterPackage>,
    pub actions: BTreeMap<String, ActionPackage>,
    pub warnings: Vec<String>,
}

impl PackageCatalog {
    pub fn scan(characters: &Path, actions: &Path) -> Self {
        let mut out = Self::default();
        out.scan_root(characters, true);
        out.scan_root(actions, false);
        out
    }

    fn scan_root(&mut self, root: &Path, character: bool) {
        for path in manifests(root) {
            match load_manifest(&path) {
                Ok(PackageManifest::Character(manifest)) if character => {
                    let dir = path.parent().unwrap().to_path_buf();
                    if let Err(error) = validate_character(&dir, &manifest) {
                        self.warnings.push(format!("{}: {error}", path.display()));
                    } else {
                        self.insert_character(dir, *manifest);
                    }
                }
                Ok(PackageManifest::Action(manifest)) if !character => {
                    let dir = path.parent().unwrap().to_path_buf();
                    let legacy_frame = manifest
                        .compatible_characters
                        .iter()
                        .find_map(|compatible| self.characters.get(&compatible.id))
                        .map(|package| {
                            (package.manifest.frame.width, package.manifest.frame.height)
                        });
                    if let Err(error) =
                        validate_action_pack_with_legacy(&dir, &manifest, legacy_frame)
                    {
                        self.warnings.push(format!("{}: {error}", path.display()));
                    } else {
                        self.insert_action(dir, manifest);
                    }
                }
                Ok(_) => self
                    .warnings
                    .push(format!("{}: package type mismatch", path.display())),
                Err(error) => self.warnings.push(format!("{}: {error}", path.display())),
            }
        }
    }

    fn insert_character(&mut self, root: PathBuf, manifest: CharacterManifest) {
        let replace = self
            .characters
            .get(&manifest.id)
            .is_none_or(|current| newer(&manifest.version, &current.manifest.version));
        if replace {
            self.characters
                .insert(manifest.id.clone(), CharacterPackage { root, manifest });
        }
    }

    fn insert_action(&mut self, root: PathBuf, manifest: ActionPackManifest) {
        let replace = self
            .actions
            .get(&manifest.id)
            .is_none_or(|current| newer(&manifest.version, &current.manifest.version));
        if replace {
            self.actions
                .insert(manifest.id.clone(), ActionPackage { root, manifest });
        }
    }

    pub fn resolve_character(&self, requested: &str) -> Option<&CharacterPackage> {
        self.characters
            .get(requested)
            .or_else(|| self.characters.values().next())
    }

    pub fn merged_actions(
        &self,
        character_id: &str,
        enabled: &[String],
    ) -> HashMap<String, RuntimeAction> {
        let Some(character) = self.resolve_character(character_id) else {
            return HashMap::new();
        };
        let mut out = character
            .manifest
            .actions
            .iter()
            .filter_map(|(id, relative)| {
                let action = runtime_action(
                    id,
                    id,
                    "semantic",
                    1,
                    0,
                    10,
                    character.root.join(relative),
                    character.manifest.render.as_ref(),
                    Some((
                        character.manifest.frame.width,
                        character.manifest.frame.height,
                    )),
                )?;
                Some((id.clone(), action))
            })
            .collect::<HashMap<_, _>>();

        let mut packs = self
            .actions
            .values()
            .filter(|package| {
                enabled.contains(&package.manifest.id)
                    && package
                        .manifest
                        .compatible_characters
                        .iter()
                        .any(|compatible| compatible.id == character.manifest.id)
            })
            .collect::<Vec<_>>();
        packs.sort_by_key(|package| package.manifest.priority.unwrap_or_default());
        for package in packs {
            for entry in &package.manifest.actions {
                if let Some(action) = runtime_action_from_pack(
                    entry,
                    package.root.join(&entry.animation),
                    package.manifest.render.as_ref(),
                    Some((
                        character.manifest.frame.width,
                        character.manifest.frame.height,
                    )),
                    package.manifest.priority,
                ) {
                    out.insert(entry.id.clone(), action);
                }
            }
        }
        out
    }

    pub fn resolve_action(
        &self,
        character_id: &str,
        enabled: &[String],
        requested: &str,
    ) -> Option<RuntimeAction> {
        let character = self.resolve_character(character_id)?;
        let actions = self.merged_actions(character_id, enabled);
        actions
            .get(requested)
            .or_else(|| actions.get(&character.manifest.default_action))
            .cloned()
    }
}

fn runtime_action_from_pack(
    entry: &ActionPackEntry,
    animation_path: PathBuf,
    render: Option<&RenderInfo>,
    legacy_frame: Option<(u32, u32)>,
    package_priority: Option<u32>,
) -> Option<RuntimeAction> {
    runtime_action(
        &entry.id,
        entry.semantic.as_deref().unwrap_or(&entry.id),
        &entry.trigger,
        entry.weight.unwrap_or(1).max(1),
        entry.cooldown_seconds.unwrap_or(0).saturating_mul(1000),
        entry
            .priority
            .unwrap_or_else(|| package_priority.unwrap_or(10).min(u8::MAX as u32) as u8),
        animation_path,
        render,
        legacy_frame,
    )
}

#[allow(clippy::too_many_arguments)]
fn runtime_action(
    id: &str,
    semantic: &str,
    trigger: &str,
    weight: u32,
    cooldown_ms: u64,
    priority: u8,
    animation_path: PathBuf,
    render: Option<&RenderInfo>,
    legacy_frame: Option<(u32, u32)>,
) -> Option<RuntimeAction> {
    let definition: AnimationDefinition =
        serde_json::from_str(&fs::read_to_string(&animation_path).ok()?).ok()?;
    let atlas_path = animation_path.parent()?.join(&definition.atlas);
    let (atlas_width, atlas_height) = png_dimensions(&atlas_path).ok()?;
    let (frame_width, frame_height) = render
        .map(|value| (value.frame_width, value.frame_height))
        .or(legacy_frame)?;
    let columns = render
        .and_then(|value| value.columns)
        .unwrap_or(atlas_width / frame_width);
    let rows = render
        .and_then(|value| value.rows)
        .unwrap_or(atlas_height / frame_height);
    Some(RuntimeAction {
        id: id.into(),
        semantic: semantic.into(),
        trigger: trigger.into(),
        weight,
        cooldown_ms,
        priority,
        animation_path,
        render: RenderSource {
            atlas_path,
            frame_width,
            frame_height,
            columns,
            rows,
        },
    })
}

fn newer(a: &str, b: &str) -> bool {
    Version::parse(a).ok() > Version::parse(b).ok()
}

fn manifests(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    fn visit(path: &Path, depth: u8, out: &mut Vec<PathBuf>) {
        if depth > 3 {
            return;
        }
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, depth + 1, out);
            } else if path.file_name().is_some_and(|name| name == "manifest.json") {
                out.push(path);
            }
        }
    }
    visit(root, 0, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> PackageCatalog {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        PackageCatalog::scan(
            &root.join("packages/characters"),
            &root.join("packages/actions"),
        )
    }

    #[test]
    fn test_missing_action_falls_back_to_default() {
        let catalog = catalog();
        let action = catalog
            .resolve_action("character.shadow-crow-ninja", &[], "missing")
            .unwrap();
        assert!(
            action.animation_path.ends_with("idle.json"),
            "{:?}",
            catalog.warnings
        );
    }

    #[test]
    fn test_action_pack_is_removed_without_dangling_runtime_reference() {
        let catalog = catalog();
        assert!(
            catalog.actions.contains_key("action.shadow-crow.office"),
            "{:?}",
            catalog.warnings
        );
        let thinking = catalog
            .resolve_action(
                "character.shadow-crow-ninja",
                &["action.shadow-crow.office".into()],
                "idle.thinking",
            )
            .expect("enabled action pack action must resolve");
        assert!(thinking.animation_path.ends_with("thinking.json"));
        assert!(thinking.render.atlas_path.ends_with("office.png"));
        assert_eq!(
            (thinking.render.frame_width, thinking.render.frame_height),
            (384, 512)
        );
        assert_eq!((thinking.render.columns, thinking.render.rows), (4, 2));
        let fallback = catalog
            .resolve_action("character.shadow-crow-ninja", &[], "idle.thinking")
            .unwrap();
        assert!(fallback.animation_path.ends_with("idle.json"));
    }

    #[test]
    fn default_action_pack_scans_successfully() {
        let catalog = catalog();
        let office = catalog
            .actions
            .get("action.shadow-crow.office")
            .unwrap_or_else(|| panic!("default action pack rejected: {:?}", catalog.warnings));
        let render = office
            .manifest
            .render
            .as_ref()
            .expect("action pack render is required");
        assert_eq!((render.frame_width, render.frame_height), (384, 512));
        assert_eq!((render.columns, render.rows), (Some(4), Some(2)));
    }
}
