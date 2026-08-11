use std::collections::HashMap;

pub use crate::animation::definition::AnimationReference;
use crate::behavior::emotion::Personality;
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
    // --- Schema v2 data-driven character profile layers ---------------------
    // All optional/defaulted so a v1 manifest still parses: the new layers
    // simply take neutral defaults. Character differences live here, never in
    // engine `if character == ...` branches.
    /// Accent color for chrome/UI theming, e.g. `"#1f2430"`.
    #[serde(default)]
    pub theme_color: Option<String>,
    #[serde(default)]
    pub identity: CharacterIdentity,
    #[serde(default, rename = "personality")]
    pub character_personality: CharacterPersonality,
    #[serde(default)]
    pub emotion_style: EmotionStyle,
    #[serde(default)]
    pub motion_style: MotionStyle,
    #[serde(default)]
    pub speech_style: SpeechStyle,
    #[serde(default)]
    pub behavior_mapping: BehaviorMapping,
    #[serde(default)]
    pub memory_profile: MemoryProfile,
    #[serde(default)]
    pub growth_profile: GrowthProfile,
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

// --- Schema v2 character profile layers -------------------------------------
//
// Each layer is an independent, defaulted slice of a character's personality.
// A v1 manifest omits them all and gets neutral defaults; a v2 manifest picks
// values per character. The engine reads these instead of branching on ids.

/// Who the character is, beyond its display name.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CharacterIdentity {
    pub role: Option<String>,
    pub tagline: Option<String>,
    pub backstory: Option<String>,
}

/// The five fixed personality dimensions, used to seed the
/// [`EmotionEngine`](crate::behavior::emotion::EmotionEngine).
///
/// Defaults to the balanced midpoint so a v1 manifest behaves like the original
/// neutral pet. [`CharacterPersonality::to_personality`] clamps each dimension
/// into `[0.0, 1.0]`.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CharacterPersonality {
    pub activity: f32,
    pub attachment: f32,
    pub curiosity: f32,
    pub playfulness: f32,
    pub sensitivity: f32,
}

impl Default for CharacterPersonality {
    fn default() -> Self {
        Self {
            activity: 0.5,
            attachment: 0.5,
            curiosity: 0.5,
            playfulness: 0.5,
            sensitivity: 0.5,
        }
    }
}

impl CharacterPersonality {
    /// Convert the manifest profile into the engine's [`Personality`], clamping
    /// each dimension into range.
    pub fn to_personality(&self) -> Personality {
        Personality::new(
            self.activity,
            self.attachment,
            self.curiosity,
            self.playfulness,
            self.sensitivity,
        )
    }
}

/// How strongly each emotion tends to be expressed or triggered.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EmotionStyle {
    pub happy_intensity: f32,
    pub annoyed_threshold: f32,
    pub sleepy_drift: f32,
    pub startle_response: f32,
}

impl Default for EmotionStyle {
    fn default() -> Self {
        Self {
            happy_intensity: 0.5,
            annoyed_threshold: 0.5,
            sleepy_drift: 0.3,
            startle_response: 0.5,
        }
    }
}

/// Motion preferences: how fast, how large, how often the character moves.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MotionStyle {
    pub speed_multiplier: f32,
    pub amplitude_multiplier: f32,
    pub idle_frequency: f32,
}

impl Default for MotionStyle {
    fn default() -> Self {
        Self {
            speed_multiplier: 1.0,
            amplitude_multiplier: 1.0,
            idle_frequency: 0.5,
        }
    }
}

/// Spoken-language style: tone, how the character addresses the user, and a
/// signature catchphrase.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SpeechStyle {
    pub tone: Option<String>,
    pub address: Option<String>,
    pub catchphrase: Option<String>,
}

/// Per-intent action overrides, so two characters can express the same
/// [`BehaviorIntent`](crate::behavior::planner::BehaviorIntent) with different
/// animations. Each field is the action id to play for that intent, or `None`
/// to keep the engine's default.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BehaviorMapping {
    pub notice_user: Option<String>,
    pub approach_user: Option<String>,
    pub avoid: Option<String>,
    pub play: Option<String>,
    pub rest: Option<String>,
    pub sleep: Option<String>,
    pub seek_attention: Option<String>,
    pub explore: Option<String>,
}

/// Memory tuning: how much the character remembers and how fast it forgets.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MemoryProfile {
    pub short_term_capacity: Option<u32>,
    pub decay_rate: Option<f32>,
    pub impression_threshold: Option<f32>,
}

/// Growth/leveling parameters. Reserved for MVP — only the structure is wired.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GrowthProfile {
    pub enabled: bool,
    pub initial_level: u32,
    pub max_level: u32,
}

impl Default for GrowthProfile {
    fn default() -> Self {
        Self {
            enabled: false,
            initial_level: 1,
            max_level: 10,
        }
    }
}

impl CharacterManifest {
    /// The [`Personality`] this character seeds the
    /// [`EmotionEngine`](crate::behavior::emotion::EmotionEngine) with.
    ///
    /// A v1 manifest has no personality layer, so this returns the balanced
    /// default — preserving the original neutral behavior.
    pub fn personality(&self) -> Personality {
        self.character_personality.to_personality()
    }

    /// Resolve the action id for a behavior intent, applying this character's
    /// overrides. `intent` is the
    /// [`BehaviorIntent`](crate::behavior::planner::BehaviorIntent) label (e.g.
    /// `"notice-user"`, also accepting the snake_case form). Returns
    /// `default_action` when the character does not override that intent.
    pub fn resolve_action<'a>(&'a self, intent: &str, default_action: &'a str) -> &'a str {
        let mapping = &self.behavior_mapping;
        let override_action = match intent {
            "notice-user" | "notice_user" => mapping.notice_user.as_deref(),
            "approach-user" | "approach_user" => mapping.approach_user.as_deref(),
            "avoid" => mapping.avoid.as_deref(),
            "play" => mapping.play.as_deref(),
            "rest" => mapping.rest.as_deref(),
            "sleep" => mapping.sleep.as_deref(),
            "seek-attention" | "seek_attention" => mapping.seek_attention.as_deref(),
            "explore" => mapping.explore.as_deref(),
            _ => None,
        };
        override_action.unwrap_or(default_action)
    }
}

#[derive(Debug, Clone)]
pub enum PackageManifest {
    Character(Box<CharacterManifest>),
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
    use super::{ActionPackManifest, CharacterManifest, Personality};

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
        assert_eq!(manifest.actions.len(), 17);
        assert_eq!(
            manifest.actions.get("clicked").map(String::as_str),
            Some("animations/clicked.json")
        );
    }

    #[test]
    fn parses_v2_action_pack_manifest() {
        let json = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packages/actions/shadow-crow-office/manifest.json"
        ));
        let manifest: ActionPackManifest = serde_json::from_str(json).unwrap();

        assert_eq!(manifest.schema_version, 2);
        assert_eq!(manifest.package_type, "action");
        assert_eq!(manifest.id, "action.shadow-crow.office");
        assert_eq!(manifest.name, "鸦影·办公动作包");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.engine_version, ">=1.0.0 <2.0.0");
        assert_eq!(manifest.priority, Some(100));
        let render = manifest.render.as_ref().unwrap();
        assert_eq!(render.atlas.as_deref(), Some("atlases/office.png"));
        assert_eq!((render.frame_width, render.frame_height), (384, 512));
        assert_eq!((render.columns, render.rows), (Some(4), Some(2)));
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

    #[test]
    fn parses_v2_character_manifest() {
        let json = r##"{
            "schemaVersion": 2,
            "packageType": "character",
            "id": "character.ninja-guardian",
            "name": "影卫",
            "version": "1.0.0",
            "engineVersion": ">=1.0.0 <2.0.0",
            "defaultAction": "idle",
            "themeColor": "#1f2430",
            "identity": {
                "role": "守护型伙伴",
                "tagline": "克制而警觉的影子守护者"
            },
            "personality": {
                "activity": 0.4,
                "attachment": 0.7,
                "curiosity": 0.3,
                "playfulness": 0.2,
                "sensitivity": 0.6
            },
            "emotionStyle": {
                "happyIntensity": 0.6,
                "annoyedThreshold": 0.5,
                "sleepyDrift": 0.3,
                "startleResponse": 0.8
            },
            "motionStyle": {
                "speedMultiplier": 0.9,
                "amplitudeMultiplier": 0.7,
                "idleFrequency": 0.4
            },
            "speechStyle": {
                "tone": "沉稳",
                "address": "主人",
                "catchphrase": "影随风动"
            },
            "behaviorMapping": {
                "noticeUser": "look",
                "rest": "relax",
                "sleep": "relax"
            },
            "memoryProfile": {
                "shortTermCapacity": 6,
                "decayRate": 0.3,
                "impressionThreshold": 0.6
            },
            "growthProfile": {
                "enabled": false,
                "initialLevel": 1,
                "maxLevel": 10
            },
            "actions": {
                "idle": "animations/idle.json"
            }
        }"##;
        let manifest: CharacterManifest = serde_json::from_str(json).unwrap();

        assert_eq!(manifest.schema_version, 2);
        assert_eq!(manifest.id, "character.ninja-guardian");
        assert_eq!(manifest.theme_color.as_deref(), Some("#1f2430"));
        assert_eq!(manifest.identity.role.as_deref(), Some("守护型伙伴"));
        assert_eq!(
            manifest.identity.tagline.as_deref(),
            Some("克制而警觉的影子守护者")
        );

        // Personality layer seeds the emotion engine, clamped into range.
        // Compared field-wise with tolerance so the test does not depend on
        // serde_json's f32 rounding of the literal decimals.
        let personality = manifest.personality();
        assert!((personality.activity - 0.4).abs() < 1e-6);
        assert!((personality.attachment - 0.7).abs() < 1e-6);
        assert!((personality.curiosity - 0.3).abs() < 1e-6);
        assert!((personality.playfulness - 0.2).abs() < 1e-6);
        assert!((personality.sensitivity - 0.6).abs() < 1e-6);

        // Emotion / motion / speech layers parse their parameterized values.
        assert_eq!(manifest.emotion_style.startle_response, 0.8);
        assert_eq!(manifest.motion_style.speed_multiplier, 0.9);
        assert_eq!(
            manifest.speech_style.catchphrase.as_deref(),
            Some("影随风动")
        );

        // Behavior overrides apply; unmapped intents fall back to the default.
        assert_eq!(manifest.resolve_action("notice-user", "look"), "look");
        assert_eq!(manifest.resolve_action("approach-user", "look"), "look");
        assert_eq!(manifest.resolve_action("rest", "relax"), "relax");
        assert_eq!(manifest.resolve_action("play", "clicked"), "clicked");

        // Memory and growth layers parse.
        assert_eq!(manifest.memory_profile.short_term_capacity, Some(6));
        assert!(!manifest.growth_profile.enabled);
        assert_eq!(manifest.growth_profile.max_level, 10);
    }

    #[test]
    fn v1_manifest_defaults_new_v2_fields() {
        // A v1 manifest omits every v2 layer; serde(default) must still parse
        // it, yielding neutral defaults and a balanced personality.
        let json = r#"{
            "schemaVersion": 1,
            "packageType": "character",
            "id": "character.legacy",
            "name": "Legacy",
            "version": "1.0.0",
            "engineVersion": ">=1.0.0 <2.0.0",
            "defaultAction": "idle",
            "actions": {
                "idle": "animations/idle.json"
            }
        }"#;
        let manifest: CharacterManifest = serde_json::from_str(json).unwrap();

        assert_eq!(manifest.schema_version, 1);
        assert!(manifest.identity.role.is_none());
        assert!(manifest.theme_color.is_none());
        // No personality layer -> balanced, the original neutral behavior.
        assert_eq!(manifest.personality(), Personality::BALANCED);
        // No behavior overrides -> every intent falls back to the default.
        assert_eq!(manifest.resolve_action("play", "clicked"), "clicked");
        assert_eq!(manifest.resolve_action("explore", "look"), "look");
        // Defaulted motion/growth layers take their struct defaults.
        assert_eq!(manifest.motion_style.speed_multiplier, 1.0);
        assert_eq!(manifest.growth_profile.initial_level, 1);
        assert_eq!(manifest.growth_profile.max_level, 10);
    }
}
