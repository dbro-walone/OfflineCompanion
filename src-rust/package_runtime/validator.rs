use super::manifest::{
    ActionPackManifest, AnimationReference, CharacterManifest, PackageManifest, RenderInfo,
};
use anyhow::{Context, Result, bail, ensure};
use semver::{Version, VersionReq};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

const FORBIDDEN: &[&str] = &[
    "exe", "dll", "com", "bat", "cmd", "ps1", "sh", "js", "vbs", "jar", "msi", "scr", "dylib", "so",
];

pub fn safe_relative(root: &Path, value: &str) -> Result<PathBuf> {
    let p = Path::new(value);
    ensure!(!p.is_absolute(), "resource path must be relative");
    ensure!(
        p.components()
            .all(|x| matches!(x, Component::Normal(_) | Component::CurDir)),
        "resource path escapes package"
    );
    if let Some(ext) = p.extension().and_then(|x| x.to_str()) {
        ensure!(
            !FORBIDDEN.contains(&ext.to_ascii_lowercase().as_str()),
            "executable resource is forbidden"
        );
    }
    Ok(root.join(p))
}

pub fn load_manifest(path: &Path) -> Result<PackageManifest> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let schema = value
        .get("schemaVersion")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    ensure!(
        (1..=2).contains(&schema),
        "unsupported schemaVersion {schema}"
    );
    let ty = value
        .get("packageType")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    let manifest = match ty {
        "character" => PackageManifest::Character(Box::new(serde_json::from_value(value)?)),
        "action" => PackageManifest::Action(Box::new(serde_json::from_value(value)?)),
        _ => bail!("unknown packageType"),
    };
    Version::parse(manifest.version()).context("invalid package version")?;
    let engine = match &manifest {
        PackageManifest::Character(x) => &x.engine_version,
        PackageManifest::Action(x) => &x.engine_version,
    };
    let normalized = if engine.contains(',') {
        engine.clone()
    } else {
        engine.split_whitespace().collect::<Vec<_>>().join(", ")
    };
    let requirement = VersionReq::parse(&normalized).context("invalid engineVersion")?;
    ensure!(
        requirement.matches(&Version::parse(env!("CARGO_PKG_VERSION"))?),
        "incompatible engineVersion"
    );
    Ok(manifest)
}

pub fn validate_character(root: &Path, m: &CharacterManifest) -> Result<()> {
    ensure!(!m.actions.is_empty(), "character has no actions");
    for path in m.actions.values() {
        let animation = safe_relative(root, path)?;
        ensure!(
            animation.is_file(),
            "missing animation {}",
            animation.display()
        );
        validate_animation(
            &animation,
            root,
            m.render.as_ref(),
            Some((m.frame.width, m.frame.height)),
        )?;
    }
    Ok(())
}
pub fn validate_action_pack(root: &Path, m: &ActionPackManifest) -> Result<()> {
    validate_action_pack_with_legacy(root, m, None)
}

pub fn validate_action_pack_with_legacy(
    root: &Path,
    m: &ActionPackManifest,
    legacy_frame: Option<(u32, u32)>,
) -> Result<()> {
    if m.schema_version >= 2 {
        ensure!(
            m.render.is_some(),
            "action pack render is required for schema v2"
        );
        let render = m.render.as_ref().expect("checked above");
        ensure!(
            render.columns.is_some() && render.rows.is_some(),
            "action pack columns and rows are required for schema v2"
        );
        let atlas = render
            .atlas
            .as_deref()
            .or(render.default_atlas.as_deref())
            .context("action pack atlas is required for schema v2")?;
        ensure!(
            safe_relative(root, atlas)?.is_file(),
            "missing declared action pack atlas"
        );
    } else {
        ensure!(
            m.render.is_some() || legacy_frame.is_some(),
            "legacy action pack needs compatible character render"
        );
    }
    for action in &m.actions {
        let animation = safe_relative(root, &action.animation)?;
        ensure!(animation.is_file(), "missing animation");
        validate_animation(&animation, root, m.render.as_ref(), legacy_frame)?;
        if m.schema_version >= 2 {
            let reference: AnimationReference =
                serde_json::from_str(&fs::read_to_string(&animation)?)?;
            let declared = m
                .render
                .as_ref()
                .and_then(|render| render.atlas.as_deref().or(render.default_atlas.as_deref()))
                .expect("validated above");
            ensure!(
                animation
                    .parent()
                    .unwrap_or(root)
                    .join(&reference.atlas)
                    .canonicalize()?
                    == safe_relative(root, declared)?.canonicalize()?,
                "animation atlas does not match action pack render"
            );
        }
    }
    Ok(())
}

fn validate_animation(
    path: &Path,
    root: &Path,
    render: Option<&RenderInfo>,
    legacy: Option<(u32, u32)>,
) -> Result<()> {
    let a: AnimationReference = serde_json::from_str(&fs::read_to_string(path)?)?;
    let atlas = path.parent().unwrap_or(root).join(&a.atlas);
    ensure!(atlas.is_file(), "missing atlas {}", atlas.display());
    let canonical_root = root.canonicalize()?;
    ensure!(
        atlas.canonicalize()?.starts_with(&canonical_root),
        "atlas escapes package"
    );
    if let Some(ext) = atlas.extension().and_then(|x| x.to_str()) {
        ensure!(
            !FORBIDDEN.contains(&ext.to_ascii_lowercase().as_str()),
            "executable resource is forbidden"
        );
    }
    let (fw, fh) = render
        .map(|x| (x.frame_width, x.frame_height))
        .or(legacy)
        .unwrap_or((0, 0));
    ensure!(fw > 0 && fh > 0, "invalid frame dimensions");
    let (w, h) = png_dimensions(&atlas)?;
    let columns = render.and_then(|x| x.columns).unwrap_or(w / fw).max(1);
    let rows = render.and_then(|x| x.rows).unwrap_or(h / fh).max(1);
    ensure!(
        fw * columns <= w && fh * rows <= h,
        "atlas geometry exceeds image"
    );
    let max = columns * rows;
    for s in [a.segments.entry, a.segments.main_loop, a.segments.exit]
        .into_iter()
        .flatten()
    {
        ensure!(
            s.start <= s.end && s.end < max,
            "animation frame out of range"
        );
    }
    Ok(())
}
pub(crate) fn png_dimensions(path: &Path) -> Result<(u32, u32)> {
    let b = fs::read(path)?;
    ensure!(
        b.len() >= 24 && &b[..8] == b"\x89PNG\r\n\x1a\n",
        "atlas is not PNG"
    );
    Ok((
        u32::from_be_bytes(b[16..20].try_into()?),
        u32::from_be_bytes(b[20..24].try_into()?),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package_runtime::manifest::ActionPackManifest;
    use uuid::Uuid;

    fn fixture_root() -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("offline-companion-validator-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("animations")).unwrap();
        fs::create_dir_all(root.join("atlases")).unwrap();
        root
    }

    fn write_test_png(path: &Path, width: u32, height: u32) {
        let mut bytes = vec![0_u8; 24];
        bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes[16..20].copy_from_slice(&width.to_be_bytes());
        bytes[20..24].copy_from_slice(&height.to_be_bytes());
        fs::write(path, bytes).unwrap();
    }
    #[test]
    fn test_manifest_rejects_executable_extension() {
        let dir = Path::new("/tmp/package");
        assert!(safe_relative(dir, "payload.exe").is_err());
        assert!(safe_relative(dir, "../escape.png").is_err());
    }
    #[test]
    fn test_manifest_rejects_out_of_range_frame() {
        let root = fixture_root();
        write_test_png(&root.join("atlases/test.png"), 32, 32);
        fs::write(
            root.join("animations/invalid.json"),
            r#"{"id":"invalid","atlas":"../atlases/test.png","fps":8,"playMode":"once","interruptible":true,"resumePolicy":"idle","mirrorable":false,"segments":{"entry":{"start":0,"end":4,"repeat":1}}}"#,
        ).unwrap();
        let manifest: ActionPackManifest = serde_json::from_str(r#"{
            "schemaVersion":2,"packageType":"action","id":"action.test","name":"test","version":"1.0.0","engineVersion":">=1.0.0 <2.0.0",
            "compatibleCharacters":[{"id":"character.test","version":">=1.0.0"}],
            "render":{"atlas":"atlases/test.png","frameWidth":16,"frameHeight":16,"columns":2,"rows":2},
            "actions":[{"id":"invalid","trigger":"idle-random","animation":"animations/invalid.json"}]
        }"#).unwrap();
        let error = validate_action_pack(&root, &manifest).unwrap_err();
        assert!(
            error.to_string().contains("frame out of range"),
            "{error:#}"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_action_pack_without_render_is_rejected_readably() {
        let root = fixture_root();
        let manifest: ActionPackManifest = serde_json::from_str(r#"{
            "schemaVersion":2,"packageType":"action","id":"action.test","name":"test","version":"1.0.0","engineVersion":">=1.0.0 <2.0.0",
            "compatibleCharacters":[{"id":"character.test","version":">=1.0.0"}],
            "actions":[{"id":"idle.test","trigger":"idle-random","animation":"animations/test.json"}]
        }"#).unwrap();
        let error = validate_action_pack(&root, &manifest).unwrap_err();
        assert_eq!(
            error.to_string(),
            "action pack render is required for schema v2"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_v1_action_pack_reuses_compatible_character_geometry() {
        let root = fixture_root();
        write_test_png(&root.join("atlases/test.png"), 32, 32);
        fs::write(
            root.join("animations/test.json"),
            r#"{"id":"idle.test","atlas":"../atlases/test.png","fps":8,"playMode":"once","interruptible":true,"resumePolicy":"idle","mirrorable":false,"segments":{"entry":{"start":0,"end":3,"repeat":1}}}"#,
        )
        .unwrap();
        let manifest: ActionPackManifest = serde_json::from_str(
            r#"{
                "schemaVersion":1,"packageType":"action","id":"action.test","name":"test","version":"1.0.0","engineVersion":">=1.0.0 <2.0.0",
                "compatibleCharacters":[{"id":"character.test","version":">=1.0.0"}],
                "actions":[{"id":"idle.test","trigger":"idle-random","animation":"animations/test.json"}]
            }"#,
        )
        .unwrap();
        validate_action_pack_with_legacy(&root, &manifest, Some((16, 16))).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
