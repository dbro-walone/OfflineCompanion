use std::collections::HashMap;

pub use crate::animation::definition::AnimationReference;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterManifest {
    pub schema_version: u32,
    pub package_type: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub engine_version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub preview: Option<String>,
    pub default_action: String,
    #[serde(default)]
    pub default_scale: Option<f64>,
    #[serde(default)]
    pub scale_range: Option<ScaleRange>,
    #[serde(default)]
    pub frame: FrameInfo,
    #[serde(default)]
    pub render: Option<RenderInfo>,
    #[serde(default)]
    pub actions: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionPackManifest {
    pub schema_version: u32,
    pub package_type: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub engine_version: String,
    #[serde(default)]
    pub compatible_characters: Vec<CompatibleCharacter>,
    #[serde(default)]
    pub priority: Option<u32>,
    #[serde(default)]
    pub render: Option<RenderInfo>,
    #[serde(default)]
    pub actions: Vec<ActionPackEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibleCharacter {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionPackEntry {
    pub id: String,
    #[serde(default)]
    pub semantic: Option<String>,
    pub trigger: String,
    #[serde(default)]
    pub weight: Option<u32>,
    #[serde(default)]
    pub cooldown_seconds: Option<u64>,
    #[serde(default)]
    pub priority: Option<u8>,
    #[serde(default)]
    pub interruptible: Option<bool>,
    pub animation: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameInfo {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderInfo {
    #[serde(default)]
    pub default_atlas: Option<String>,
    #[serde(default)]
    pub atlas: Option<String>,
    pub frame_width: u32,
    pub frame_height: u32,
    #[serde(default)]
    pub columns: Option<u32>,
    #[serde(default)]
    pub rows: Option<u32>,
    #[serde(default)]
    pub default_scale: Option<f64>,
    #[serde(default)]
    pub scale_range: Option<ScaleRange>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaleRange {
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone)]
pub enum PackageManifest {
    Character(CharacterManifest),
    Action(ActionPackManifest),
}

impl PackageManifest {
    pub fn id(&self) -> &str {
        match self {
            Self::Character(x) => &x.id,
            Self::Action(x) => &x.id,
        }
    }
    pub fn version(&self) -> &str {
        match self {
            Self::Character(x) => &x.version,
            Self::Action(x) => &x.version,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ActionPackManifest, CharacterManifest};

    #[test]
    fn parses_v1_character_manifest() {
        let json = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/characters/shadow-crow-ninja/manifest.json"
        ));
        let manifest: CharacterManifest = serde_json::from_str(json).unwrap();

        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.package_type, "character");
        assert_eq!(manifest.id, "character.shadow-crow-ninja");
        assert_eq!(manifest.name, "鸦影");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.engine_version, ">=1.0.0 <2.0.0");
        assert_eq!(manifest.author.as_deref(), Some("Local"));
        assert_eq!(manifest.license.as_deref(), Some("personal-use"));
        assert_eq!(manifest.preview.as_deref(), Some("preview.png"));
        assert_eq!(manifest.default_action, "idle");
        assert_eq!(manifest.default_scale, Some(1.0));
        let scale_range = manifest.scale_range.unwrap();
        assert_eq!(scale_range.min, 0.75);
        assert_eq!(scale_range.max, 1.4);
        assert_eq!(manifest.frame.width, 384);
        assert_eq!(manifest.frame.height, 512);
        assert!(manifest.render.is_none());
        assert_eq!(manifest.actions.len(), 12);
        assert_eq!(
            manifest.actions.get("clicked").map(String::as_str),
            Some("animations/clicked.json")
        );
    }

    #[test]
    fn parses_v1_action_pack_manifest() {
        let json = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/actions/shadow-crow-office/manifest.json"
        ));
        let manifest: ActionPackManifest = serde_json::from_str(json).unwrap();

        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.package_type, "action");
        assert_eq!(manifest.id, "action.shadow-crow.office");
        assert_eq!(manifest.name, "鸦影·办公动作包");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.engine_version, ">=1.0.0 <2.0.0");
        assert_eq!(manifest.priority, Some(100));
        assert!(manifest.render.is_none());
        assert_eq!(manifest.compatible_characters.len(), 1);
        assert_eq!(
            manifest.compatible_characters[0].id,
            "character.shadow-crow-ninja"
        );
        assert_eq!(manifest.compatible_characters[0].version, ">=1.0.0 <2.0.0");

        assert_eq!(manifest.actions.len(), 1);
        let action = &manifest.actions[0];
        assert_eq!(action.id, "idle.thinking");
        assert!(action.semantic.is_none());
        assert_eq!(action.trigger, "idle-random");
        assert_eq!(action.weight, Some(20));
        assert_eq!(action.cooldown_seconds, Some(180));
        assert!(action.priority.is_none());
        assert!(action.interruptible.is_none());
        assert_eq!(action.animation, "animations/thinking.json");
    }
}
