use super::definition::{AnimationSegments, PlayMode, Segment};

const MAX_SEGMENT_REPEAT: u32 = 100;

pub fn expand_frames(segments: &AnimationSegments, play_mode: &PlayMode) -> Vec<u32> {
    let entry = segments
        .entry
        .as_ref()
        .map(expand_repeated)
        .unwrap_or_default();
    let main_loop = segments
        .main_loop
        .as_ref()
        .map(expand_once)
        .unwrap_or_default();
    let exit = segments
        .exit
        .as_ref()
        .map(expand_repeated)
        .unwrap_or_default();

    let mut frames = entry.clone();
    match play_mode {
        PlayMode::Once => {
            frames.extend(main_loop);
            frames.extend(exit);
        }
        PlayMode::Loop | PlayMode::HoldLast => frames.extend(main_loop),
        PlayMode::PingPong => {
            frames.extend(main_loop.iter().copied());
            frames.extend(main_loop.into_iter().rev());
        }
        PlayMode::ReverseReturn => {
            frames.extend(main_loop);
            frames.extend(entry.into_iter().rev());
        }
    }
    frames
}

fn expand_once(segment: &Segment) -> Vec<u32> {
    (segment.start..=segment.end).collect()
}

fn expand_repeated(segment: &Segment) -> Vec<u32> {
    let repeat = segment.repeat.clamp(1, MAX_SEGMENT_REPEAT);
    let iteration = expand_once(segment);
    iteration.repeat(repeat as usize)
}

#[cfg(test)]
mod tests {
    use super::expand_frames;
    use crate::animation::definition::{AnimationDefinition, AnimationSegments, PlayMode, Segment};

    fn parse_fixture(path: &str) -> AnimationDefinition {
        let root = env!("CARGO_MANIFEST_DIR");
        let json = std::fs::read_to_string(format!("{root}/{path}")).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    fn fixture_frames(path: &str) -> Vec<u32> {
        let definition = parse_fixture(path);
        expand_frames(&definition.segments, &definition.play_mode)
    }

    #[test]
    fn expands_real_animation_fixtures() {
        let cases = [
            (
                "packages/characters/shadow-crow-ninja/animations/clicked.json",
                vec![4, 5, 3],
            ),
            (
                "packages/characters/shadow-crow-ninja/animations/idle.json",
                vec![0, 1, 2, 3],
            ),
            (
                "packages/characters/shadow-crow-ninja/animations/celebrate.json",
                vec![7, 7, 7, 3],
            ),
            (
                "packages/characters/shadow-crow-ninja/animations/focus.json",
                vec![1, 2],
            ),
            (
                "packages/characters/shadow-crow-ninja/animations/relax.json",
                vec![1, 2, 3, 3, 2, 1],
            ),
            (
                "packages/characters/shadow-crow-ninja/animations/edge-left.json",
                vec![6],
            ),
            (
                "packages/characters/shadow-crow-ninja/animations/reminder.json",
                vec![6, 6, 6, 3],
            ),
            (
                "packages/actions/shadow-crow-office/animations/thinking.json",
                vec![1, 2, 1, 2, 3],
            ),
        ];

        for (path, expected) in cases {
            assert_eq!(fixture_frames(path), expected, "fixture: {path}");
        }
    }

    #[test]
    fn treats_zero_repeat_as_one() {
        let segments = AnimationSegments {
            entry: Some(Segment {
                start: 4,
                end: 5,
                repeat: 0,
            }),
            main_loop: None,
            exit: None,
        };

        assert_eq!(expand_frames(&segments, &PlayMode::Once), vec![4, 5]);
    }

    #[test]
    fn skips_missing_entry_and_exit() {
        let segments = AnimationSegments {
            entry: None,
            main_loop: Some(Segment {
                start: 1,
                end: 3,
                repeat: 1,
            }),
            exit: None,
        };

        assert_eq!(expand_frames(&segments, &PlayMode::Once), vec![1, 2, 3]);
    }

    #[test]
    fn clamps_repeat_to_one_hundred() {
        let segments = AnimationSegments {
            entry: Some(Segment {
                start: 7,
                end: 7,
                repeat: 101,
            }),
            main_loop: None,
            exit: None,
        };

        assert_eq!(expand_frames(&segments, &PlayMode::Once), vec![7; 100]);
    }
}
