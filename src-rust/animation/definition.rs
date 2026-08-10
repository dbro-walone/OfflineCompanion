use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PlayMode {
    Once,
    Loop,
    PingPong,
    HoldLast,
    ReverseReturn,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ResumePolicy {
    Restart,
    Idle,
    Resume,
    Previous,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationDefinition {
    pub id: String,
    pub atlas: String,
    pub fps: f32,
    pub play_mode: PlayMode,
    pub interruptible: bool,
    pub resume_policy: ResumePolicy,
    pub mirrorable: bool,
    pub segments: AnimationSegments,
}

impl AnimationDefinition {
    pub fn frame_duration_ms(&self) -> u64 {
        (1000.0 / self.fps.clamp(1.0, 120.0)).round() as u64
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationSegments {
    #[serde(default)]
    pub entry: Option<Segment>,
    #[serde(default, rename = "loop")]
    pub main_loop: Option<Segment>,
    #[serde(default)]
    pub exit: Option<Segment>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub start: u32,
    pub end: u32,
    #[serde(default)]
    pub repeat: u32,
}

pub type AnimationReference = AnimationDefinition;

#[cfg(test)]
mod tests {
    use super::{AnimationDefinition, PlayMode, ResumePolicy};

    fn parse_fixture(path: &str) -> AnimationDefinition {
        let root = env!("CARGO_MANIFEST_DIR");
        let json = std::fs::read_to_string(format!("{root}/{path}")).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn parses_clicked_animation() {
        let definition =
            parse_fixture("packages/characters/shadow-crow-ninja/animations/clicked.json");

        assert_eq!(definition.id, "clicked");
        assert_eq!(definition.atlas, "../atlases/base.png");
        assert_eq!(definition.fps, 6.0);
        assert_eq!(definition.play_mode, PlayMode::Once);
        assert!(definition.interruptible);
        assert_eq!(definition.resume_policy, ResumePolicy::Previous);
        assert!(definition.mirrorable);
        let entry = definition.segments.entry.unwrap();
        assert_eq!((entry.start, entry.end, entry.repeat), (4, 5, 1));
        assert!(definition.segments.main_loop.is_none());
        let exit = definition.segments.exit.unwrap();
        assert_eq!((exit.start, exit.end, exit.repeat), (3, 3, 1));
    }

    #[test]
    fn parses_idle_animation() {
        let definition =
            parse_fixture("packages/characters/shadow-crow-ninja/animations/idle.json");

        assert_eq!(definition.id, "idle");
        assert_eq!(definition.fps, 2.0);
        assert_eq!(definition.play_mode, PlayMode::Loop);
        assert!(definition.segments.entry.is_none());
        let main_loop = definition.segments.main_loop.unwrap();
        assert_eq!(
            (main_loop.start, main_loop.end, main_loop.repeat),
            (0, 3, 1)
        );
        assert!(definition.segments.exit.is_none());
    }
}
