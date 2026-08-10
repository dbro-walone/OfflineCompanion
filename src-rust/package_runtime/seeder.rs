use anyhow::Result;
use std::{fs, path::Path};
const FILES: &[(&str, &[u8])] = &[
    (
        "characters/shadow-crow-ninja/manifest.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/manifest.json"
        )),
    ),
    (
        "characters/shadow-crow-ninja/atlases/base.png",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/atlases/base.png"
        )),
    ),
    (
        "characters/shadow-crow-ninja/preview.png",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/preview.png"
        )),
    ),
    (
        "characters/shadow-crow-ninja/icon.png",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/icon.png"
        )),
    ),
    (
        "characters/shadow-crow-ninja/animations/idle.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/animations/idle.json"
        )),
    ),
    (
        "characters/shadow-crow-ninja/animations/clicked.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/animations/clicked.json"
        )),
    ),
    (
        "characters/shadow-crow-ninja/animations/head-pat.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/animations/head-pat.json"
        )),
    ),
    (
        "characters/shadow-crow-ninja/animations/look.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/animations/look.json"
        )),
    ),
    (
        "characters/shadow-crow-ninja/animations/dragged.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/animations/dragged.json"
        )),
    ),
    (
        "characters/shadow-crow-ninja/animations/reminder.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/animations/reminder.json"
        )),
    ),
    (
        "characters/shadow-crow-ninja/animations/celebrate.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/animations/celebrate.json"
        )),
    ),
    (
        "characters/shadow-crow-ninja/animations/focus.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/animations/focus.json"
        )),
    ),
    (
        "characters/shadow-crow-ninja/animations/relax.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/animations/relax.json"
        )),
    ),
    (
        "characters/shadow-crow-ninja/animations/fall.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/animations/fall.json"
        )),
    ),
    (
        "characters/shadow-crow-ninja/animations/landing.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/animations/landing.json"
        )),
    ),
    (
        "characters/shadow-crow-ninja/animations/startled.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/animations/startled.json"
        )),
    ),
    (
        "characters/shadow-crow-ninja/animations/edge-left.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/animations/edge-left.json"
        )),
    ),
    (
        "characters/shadow-crow-ninja/animations/edge-right.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/animations/edge-right.json"
        )),
    ),
    (
        "actions/shadow-crow-office/manifest.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/actions/shadow-crow-office/manifest.json"
        )),
    ),
    (
        "actions/shadow-crow-office/animations/thinking.json",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/actions/shadow-crow-office/animations/thinking.json"
        )),
    ),
    (
        "actions/shadow-crow-office/atlases/office.png",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/actions/shadow-crow-office/atlases/office.png"
        )),
    ),
];
pub fn seed_defaults(root: &Path) -> Result<()> {
    for (rel, bytes) in FILES {
        let path = root.join(rel);
        if fs::read(&path).is_ok_and(|current| current == *bytes) {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?
        }
        let temp = path.with_extension("seed-new.tmp");
        let backup = path.with_extension("seed-old.tmp");
        fs::write(&temp, bytes)?;
        if backup.exists() {
            fs::remove_file(&backup)?;
        }
        if path.exists() {
            fs::rename(&path, &backup)?;
        }
        if let Err(error) = fs::rename(&temp, &path) {
            if backup.exists() {
                let _ = fs::rename(&backup, &path);
            }
            return Err(error.into());
        }
        if backup.exists() {
            fs::remove_file(backup)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_packages_are_migrated_and_new_actions_are_seeded() {
        let data_root =
            std::env::temp_dir().join(format!("offline-seeder-{}", uuid::Uuid::new_v4()));
        let root = data_root.join("packages");
        let manifest = root.join("characters/shadow-crow-ninja/manifest.json");
        fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        fs::write(&manifest, b"old built-in manifest").unwrap();
        seed_defaults(&root).unwrap();
        assert_eq!(fs::read(&manifest).unwrap(), FILES[0].1);
        assert!(
            root.join("characters/shadow-crow-ninja/animations/landing.json")
                .is_file()
        );
        assert!(
            root.join("actions/shadow-crow-office/manifest.json")
                .is_file()
        );
        let catalog = crate::package_runtime::catalog::PackageCatalog::scan(
            &root.join("characters"),
            &root.join("actions"),
        );
        assert!(
            catalog
                .characters
                .contains_key("character.shadow-crow-ninja")
        );
        assert!(catalog.actions.contains_key("action.shadow-crow.office"));
        fs::remove_dir_all(data_root).unwrap();
    }
}
