use super::{
    definition::{AnimationDefinition, PlayMode, ResumePolicy},
    sequence::expand_frames,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerStatus {
    Playing,
    Completed,
}
#[derive(Debug, Clone)]
pub struct AnimationPlayer {
    definition: AnimationDefinition,
    frames: Vec<u32>,
    cursor: usize,
    next_frame_ms: u64,
    status: PlayerStatus,
}
impl AnimationPlayer {
    pub fn new(definition: AnimationDefinition, now_ms: u64) -> Self {
        let frames = expand_frames(&definition.segments, &definition.play_mode);
        Self {
            next_frame_ms: now_ms + definition.frame_duration_ms(),
            definition,
            frames,
            cursor: 0,
            status: PlayerStatus::Playing,
        }
    }
    pub fn frame(&self) -> Option<u32> {
        self.frames.get(self.cursor).copied()
    }
    pub fn status(&self) -> PlayerStatus {
        self.status
    }
    pub fn interruptible(&self) -> bool {
        self.definition.interruptible
    }
    pub fn resume_policy(&self) -> &ResumePolicy {
        &self.definition.resume_policy
    }
    pub fn safe_to_interrupt(&self) -> bool {
        self.definition.interruptible
            || self.status == PlayerStatus::Completed
            || self
                .frames
                .len()
                .checked_sub(1)
                .is_some_and(|last| self.cursor >= last)
    }
    pub fn tick(&mut self, now_ms: u64) -> Option<u32> {
        if self.status == PlayerStatus::Completed
            || self.frames.is_empty()
            || now_ms < self.next_frame_ms
        {
            return None;
        }
        let step =
            ((now_ms - self.next_frame_ms) / self.definition.frame_duration_ms() + 1) as usize;
        self.next_frame_ms = self
            .next_frame_ms
            .saturating_add(step as u64 * self.definition.frame_duration_ms());
        let end = self.frames.len() - 1;
        match self.definition.play_mode {
            PlayMode::Loop | PlayMode::PingPong => {
                self.cursor = (self.cursor + step) % self.frames.len()
            }
            PlayMode::HoldLast => self.cursor = (self.cursor + step).min(end),
            PlayMode::Once | PlayMode::ReverseReturn => {
                self.cursor += step;
                if self.cursor >= end {
                    self.cursor = end;
                    self.status = PlayerStatus::Completed
                }
            }
        }
        self.frame()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::definition::*;
    fn def(mode: PlayMode) -> AnimationDefinition {
        AnimationDefinition {
            id: "x".into(),
            atlas: "x.png".into(),
            fps: 8.,
            play_mode: mode,
            interruptible: true,
            resume_policy: ResumePolicy::Restart,
            mirrorable: true,
            segments: AnimationSegments {
                entry: None,
                main_loop: Some(Segment {
                    start: 0,
                    end: 2,
                    repeat: 1,
                }),
                exit: None,
            },
        }
    }
    #[test]
    fn fps_eight_uses_125ms() {
        assert_eq!(def(PlayMode::Once).frame_duration_ms(), 125)
    }
    #[test]
    fn once_completes_and_holds_last() {
        let mut p = AnimationPlayer::new(def(PlayMode::Once), 0);
        assert_eq!(p.tick(125), Some(1));
        assert_eq!(p.tick(250), Some(2));
        assert_eq!(p.status(), PlayerStatus::Completed)
    }
    #[test]
    fn loop_wraps() {
        let mut p = AnimationPlayer::new(def(PlayMode::Loop), 0);
        assert_eq!(p.tick(375), Some(0));
    }
    #[test]
    fn hold_last_stays_playing() {
        let mut p = AnimationPlayer::new(def(PlayMode::HoldLast), 0);
        p.tick(999);
        assert_eq!(p.frame(), Some(2));
        assert_eq!(p.status(), PlayerStatus::Playing)
    }
    #[test]
    fn test_non_interruptible_action_waits_for_safe_boundary() {
        let mut definition = def(PlayMode::HoldLast);
        definition.interruptible = false;
        let mut player = AnimationPlayer::new(definition, 0);
        assert!(!player.safe_to_interrupt());
        player.tick(125);
        assert!(!player.safe_to_interrupt());
        player.tick(250);
        assert!(player.safe_to_interrupt());
    }
}
