use super::{
    manifest::{ActionPackManifest, CharacterManifest, PackageManifest},
    validator::{load_manifest, validate_action_pack, validate_character},
};
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
                Ok(PackageManifest::Character(m)) if character => {
                    let dir = path.parent().unwrap().to_path_buf();
                    if let Err(e) = validate_character(&dir, &m) {
                        self.warnings.push(format!("{}: {e}", path.display()));
                        continue;
                    }
                    self.insert_character(dir, m)
                }
                Ok(PackageManifest::Action(m)) if !character => {
                    let dir = path.parent().unwrap().to_path_buf();
                    if let Err(e) = validate_action_pack(&dir, &m) {
                        self.warnings.push(format!("{}: {e}", path.display()));
                        continue;
                    }
                    self.insert_action(dir, m)
                }
                Ok(_) => self
                    .warnings
                    .push(format!("{}: package type mismatch", path.display())),
                Err(e) => self.warnings.push(format!("{}: {e}", path.display())),
            }
        }
    }
    fn insert_character(&mut self, root: PathBuf, m: CharacterManifest) {
        let replace = self
            .characters
            .get(&m.id)
            .is_none_or(|x| newer(&m.version, &x.manifest.version));
        if replace {
            self.characters
                .insert(m.id.clone(), CharacterPackage { root, manifest: m });
        }
    }
    fn insert_action(&mut self, root: PathBuf, m: ActionPackManifest) {
        let replace = self
            .actions
            .get(&m.id)
            .is_none_or(|x| newer(&m.version, &x.manifest.version));
        if replace {
            self.actions
                .insert(m.id.clone(), ActionPackage { root, manifest: m });
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
    ) -> HashMap<String, PathBuf> {
        let Some(c) = self.resolve_character(character_id) else {
            return HashMap::new();
        };
        let mut out = c
            .manifest
            .actions
            .iter()
            .map(|(id, p)| (id.clone(), c.root.join(p)))
            .collect::<HashMap<_, _>>();
        let mut packs = self
            .actions
            .values()
            .filter(|x| {
                enabled.contains(&x.manifest.id)
                    && x.manifest
                        .compatible_characters
                        .iter()
                        .any(|y| y.id == c.manifest.id)
            })
            .collect::<Vec<_>>();
        packs.sort_by_key(|x| x.manifest.priority.unwrap_or_default());
        for p in packs {
            for a in &p.manifest.actions {
                out.insert(a.id.clone(), p.root.join(&a.animation));
            }
        }
        out
    }
    pub fn resolve_action(
        &self,
        character_id: &str,
        enabled: &[String],
        requested: &str,
    ) -> Option<PathBuf> {
        let c = self.resolve_character(character_id)?;
        let actions = self.merged_actions(character_id, enabled);
        actions
            .get(requested)
            .or_else(|| actions.get(&c.manifest.default_action))
            .cloned()
    }
}
fn newer(a: &str, b: &str) -> bool {
    Version::parse(a).ok() > Version::parse(b).ok()
}
fn manifests(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    fn visit(p: &Path, depth: u8, out: &mut Vec<PathBuf>) {
        if depth > 3 {
            return;
        }
        let Ok(entries) = fs::read_dir(p) else { return };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                visit(&path, depth + 1, out)
            } else if path.file_name().is_some_and(|x| x == "manifest.json") {
                out.push(path)
            }
        }
    }
    visit(root, 0, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_missing_action_falls_back_to_default() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let c = PackageCatalog::scan(
            &root.join("packages/characters"),
            &root.join("packages/actions"),
        );
        assert!(
            c.resolve_action("character.shadow-crow-ninja", &[], "missing")
                .is_some(),
            "{:?}",
            c.warnings
        );
    }
    #[test]
    fn test_action_pack_is_removed_without_dangling_runtime_reference() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let c = PackageCatalog::scan(
            &root.join("packages/characters"),
            &root.join("packages/actions"),
        );
        assert!(
            c.resolve_action(
                "character.shadow-crow-ninja",
                &["action.shadow-crow.office".into()],
                "idle.thinking"
            )
            .is_some()
        );
        assert!(
            c.resolve_action("character.shadow-crow-ninja", &[], "idle.thinking")
                .unwrap()
                .ends_with("idle.json")
        );
    }
}
