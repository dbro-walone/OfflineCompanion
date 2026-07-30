use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    rc::Rc,
    time::Duration,
};

use anyhow::{Context, Result};
use serde::Deserialize;
use slint::{ComponentHandle, Timer, TimerMode};

use crate::PetWindow;

pub const WAVE: &[i32] = &[4, 5, 4, 5, 3];
pub const JUMP: &[i32] = &[0, 2, 6, 2, 0];
pub const CRY: &[i32] = &[7, 7, 4, 7, 4];

const MANIFEST: &str = include_str!("../packages/characters/shadow-crow-ninja/manifest.json");
const IDLE: &str = include_str!("../packages/characters/shadow-crow-ninja/animations/idle.json");
const CLICKED: &str =
    include_str!("../packages/characters/shadow-crow-ninja/animations/clicked.json");
const DRAGGED: &str =
    include_str!("../packages/characters/shadow-crow-ninja/animations/dragged.json");
const REMINDER: &str =
    include_str!("../packages/characters/shadow-crow-ninja/animations/reminder.json");
const CELEBRATE: &str =
    include_str!("../packages/characters/shadow-crow-ninja/animations/celebrate.json");
const FOCUS: &str = include_str!("../packages/characters/shadow-crow-ninja/animations/focus.json");
const RELAX: &str = include_str!("../packages/characters/shadow-crow-ninja/animations/relax.json");
const EDGE_LEFT: &str =
    include_str!("../packages/characters/shadow-crow-ninja/animations/edge-left.json");
const EDGE_RIGHT: &str =
    include_str!("../packages/characters/shadow-crow-ninja/animations/edge-right.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    actions: HashMap<String, String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnimationSpec {
    fps: u32,
    play_mode: String,
    segments: BTreeMap<String, Segment>,
}

#[derive(Clone, Deserialize)]
struct Segment {
    start: i32,
    end: i32,
    repeat: usize,
}

struct Playback {
    frames: Vec<i32>,
    index: usize,
    loops_remaining: usize,
    hold_last: bool,
    on_finished: Option<Box<dyn FnOnce()>>,
}

#[derive(Clone)]
pub struct AnimationPlayer {
    specs: Rc<HashMap<String, AnimationSpec>>,
    playback: Rc<RefCell<Option<Playback>>>,
    timer: Rc<Timer>,
    weak_pet: slint::Weak<PetWindow>,
}

impl AnimationPlayer {
    pub fn load(pet: &PetWindow) -> Result<Self> {
        let manifest: Manifest =
            serde_json::from_str(MANIFEST).context("invalid character manifest")?;
        let mut specs = HashMap::new();
        for (name, path) in manifest.actions {
            let source = animation_source(&path)
                .with_context(|| format!("unknown animation path {path}"))?;
            let spec = serde_json::from_str(source)
                .with_context(|| format!("invalid animation spec {path}"))?;
            specs.insert(name, spec);
        }
        Ok(Self {
            specs: Rc::new(specs),
            playback: Rc::new(RefCell::new(None)),
            timer: Rc::new(Timer::default()),
            weak_pet: pet.as_weak(),
        })
    }

    pub fn play(&self, name: &str) {
        let Some(spec) = self.specs.get(name) else {
            return;
        };
        let frames = frames_for(spec);
        if frames.is_empty() {
            return;
        }
        let loops = if matches!(spec.play_mode.as_str(), "loop" | "ping-pong") {
            usize::MAX
        } else {
            1
        };
        self.start(
            frames,
            loops,
            Duration::from_millis(1_000 / u64::from(spec.fps.max(1))),
            spec.play_mode == "hold-last",
            None,
        );
    }

    pub fn play_sequence(&self, frames: &[i32], loops: usize) {
        self.start(
            frames.to_vec(),
            loops.max(1),
            Duration::from_millis(140),
            false,
            None,
        );
    }

    fn start(
        &self,
        frames: Vec<i32>,
        loops_remaining: usize,
        frame_duration: Duration,
        hold_last: bool,
        on_finished: Option<Box<dyn FnOnce()>>,
    ) {
        if frames.is_empty() {
            return;
        }
        *self.playback.borrow_mut() = Some(Playback {
            frames,
            index: 0,
            loops_remaining,
            hold_last,
            on_finished,
        });
        let playback = self.playback.clone();
        let weak_pet = self.weak_pet.clone();
        let weak_timer = Rc::downgrade(&self.timer);
        self.timer
            .start(TimerMode::Repeated, frame_duration, move || {
                let Some(pet) = weak_pet.upgrade() else {
                    return;
                };
                let mut active = playback.borrow_mut();
                let Some(state) = active.as_mut() else {
                    return;
                };
                pet.set_frame_index(state.frames[state.index]);
                state.index += 1;
                if state.index < state.frames.len() {
                    return;
                }
                if state.loops_remaining > 1 {
                    if state.loops_remaining != usize::MAX {
                        state.loops_remaining -= 1;
                    }
                    state.index = 0;
                } else {
                    let hold_last = state.hold_last;
                    let on_finished = active
                        .take()
                        .and_then(|mut finished| finished.on_finished.take());
                    drop(active);
                    if !hold_last {
                        pet.set_frame_index(0);
                    }
                    if let Some(timer) = weak_timer.upgrade() {
                        timer.stop();
                    }
                    if let Some(on_finished) = on_finished {
                        on_finished();
                    }
                }
            });
    }

    pub fn stop(&self) {
        self.playback.borrow_mut().take();
        self.timer.stop();
        if let Some(pet) = self.weak_pet.upgrade() {
            pet.set_frame_index(0);
        }
    }
}

fn animation_source(path: &str) -> Option<&'static str> {
    match path {
        "animations/idle.json" => Some(IDLE),
        "animations/clicked.json" => Some(CLICKED),
        "animations/dragged.json" => Some(DRAGGED),
        "animations/reminder.json" => Some(REMINDER),
        "animations/celebrate.json" => Some(CELEBRATE),
        "animations/focus.json" => Some(FOCUS),
        "animations/relax.json" => Some(RELAX),
        "animations/edge-left.json" => Some(EDGE_LEFT),
        "animations/edge-right.json" => Some(EDGE_RIGHT),
        _ => None,
    }
}

fn frames_for(spec: &AnimationSpec) -> Vec<i32> {
    let mut frames = Vec::new();
    for key in ["entry", "loop", "exit"] {
        if let Some(segment) = spec.segments.get(key) {
            append_segment(&mut frames, segment);
        }
    }
    if spec.play_mode == "ping-pong" && frames.len() > 2 {
        let reverse = frames[1..frames.len() - 1]
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>();
        frames.extend(reverse);
    }
    frames
}

fn append_segment(frames: &mut Vec<i32>, segment: &Segment) {
    let range = if segment.start <= segment.end {
        (segment.start..=segment.end).collect::<Vec<_>>()
    } else {
        (segment.end..=segment.start).rev().collect::<Vec<_>>()
    };
    for _ in 0..segment.repeat {
        frames.extend_from_slice(&range);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_all_manifest_actions_and_expands_clicked() {
        let manifest: Manifest = serde_json::from_str(MANIFEST).unwrap();
        assert_eq!(manifest.actions.len(), 12);
        let spec: AnimationSpec = serde_json::from_str(CLICKED).unwrap();
        assert_eq!(frames_for(&spec), vec![4, 5, 3]);
    }

    #[test]
    fn ping_pong_does_not_duplicate_turning_frames() {
        let spec: AnimationSpec = serde_json::from_str(RELAX).unwrap();
        assert_eq!(frames_for(&spec), vec![1, 2, 3, 2]);
    }
}
