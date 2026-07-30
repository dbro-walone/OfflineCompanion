use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PlayMode {
    #[default]
    Loop,
    Once,
    HoldLast,
    PingPong,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SegmentDef {
    pub start: i32,
    pub end: i32,
    #[serde(default = "default_repeat")]
    pub repeat: u32,
}

fn default_repeat() -> u32 {
    1
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnimationDef {
    pub fps: u32,
    #[serde(default)]
    pub play_mode: PlayMode,
    pub segments: BTreeMap<String, SegmentDef>,
}

impl AnimationDef {
    /// 按 segments 的 key 字母序展开成完整帧序列。
    pub(crate) fn frames(&self) -> Vec<i32> {
        let mut frames = Vec::new();
        for segment in self.segments.values() {
            for _ in 0..segment.repeat {
                if segment.start <= segment.end {
                    frames.extend(segment.start..=segment.end);
                } else {
                    frames.extend((segment.end..=segment.start).rev());
                }
            }
        }
        if frames.is_empty() {
            frames.push(0);
        }
        frames
    }

    /// 每帧时长（毫秒）。fps 为 0 时退化为 200ms，避免除零。
    fn frame_ms(&self) -> u64 {
        if self.fps == 0 {
            200
        } else {
            1000 / self.fps as u64
        }
    }
}

/// 给定动画定义与自开播以来的耗时，返回 (当前帧索引, 是否已播完一次)。
/// loop / ping-pong 永不完成；once / hold-last 在走到末帧后完成。
pub(crate) fn sample(def: &AnimationDef, elapsed: std::time::Duration) -> (i32, bool) {
    let frames = def.frames();
    let len = frames.len();
    let tick = (elapsed.as_millis() / def.frame_ms().max(1) as u128) as usize;
    let (index, completed) = match def.play_mode {
        PlayMode::Loop => (tick % len, false),
        PlayMode::PingPong => {
            if len == 1 {
                (0, false)
            } else {
                let period = 2 * (len - 1);
                let m = tick % period;
                let idx = if m < len { m } else { period - m };
                (idx, false)
            }
        }
        PlayMode::Once | PlayMode::HoldLast => {
            if tick >= len {
                (len - 1, true)
            } else {
                (tick, false)
            }
        }
    };
    (frames[index.min(len - 1)], completed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn def(play_mode: PlayMode, fps: u32, start: i32, end: i32) -> AnimationDef {
        let mut segments = BTreeMap::new();
        segments.insert(
            "loop".to_string(),
            SegmentDef {
                start,
                end,
                repeat: 1,
            },
        );
        AnimationDef {
            fps,
            play_mode,
            segments,
        }
    }

    #[test]
    fn expands_ascending_and_descending() {
        let d = def(PlayMode::Loop, 2, 0, 3);
        assert_eq!(d.frames(), vec![0, 1, 2, 3]);
        let mut seg = BTreeMap::new();
        seg.insert(
            "a".to_string(),
            SegmentDef {
                start: 5,
                end: 3,
                repeat: 1,
            },
        );
        let d2 = AnimationDef {
            fps: 2,
            play_mode: PlayMode::Loop,
            segments: seg,
        };
        assert_eq!(d2.frames(), vec![5, 4, 3]);
    }

    #[test]
    fn loop_never_completes_and_wraps() {
        let d = def(PlayMode::Loop, 10, 0, 1);
        let frame_ms = d.frame_ms();
        let (f, done) = sample(&d, Duration::from_millis(0));
        assert_eq!(f, 0);
        assert!(!done);
        let (f, _) = sample(&d, Duration::from_millis(frame_ms));
        assert_eq!(f, 1);
        let (f, done) = sample(&d, Duration::from_millis(frame_ms * 2));
        assert_eq!(f, 0);
        assert!(!done);
    }

    #[test]
    fn once_completes_and_holds_last() {
        let d = def(PlayMode::Once, 10, 0, 2);
        let frame_ms = d.frame_ms();
        assert_eq!(sample(&d, Duration::from_millis(0)).0, 0);
        assert_eq!(sample(&d, Duration::from_millis(frame_ms)).0, 1);
        let (f, done) = sample(&d, Duration::from_millis(frame_ms * 3));
        assert_eq!(f, 2);
        assert!(done);
    }

    #[test]
    fn pingpong_reverses_without_completing() {
        let d = def(PlayMode::PingPong, 10, 0, 2);
        let ms = d.frame_ms();
        assert_eq!(sample(&d, Duration::ZERO).0, 0);
        assert_eq!(sample(&d, Duration::from_millis(ms * 2)).0, 2);
        assert_eq!(sample(&d, Duration::from_millis(ms * 4)).0, 0);
        assert!(!sample(&d, Duration::from_millis(ms * 4)).1);
    }
}
