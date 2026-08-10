#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReminderKind {
    Default,
    Todo,
    Timer,
    Sedentary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitRegion {
    Head,
    Body,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PetEvent {
    AppStarted,
    PointerEntered {
        distance_px: f32,
    },
    PointerNear {
        distance_px: f32,
    },
    PointerExited,
    PetClicked {
        region: HitRegion,
        click_count: u8,
    },
    DragStarted {
        pointer_x: f32,
        pointer_y: f32,
    },
    DragMoved {
        dx: f32,
        dy: f32,
        velocity_x: f32,
        velocity_y: f32,
    },
    DragReleased {
        velocity_x: f32,
        velocity_y: f32,
        x: i32,
        y: i32,
    },
    ReminderRaised {
        kind: ReminderKind,
        id: String,
    },
    ReminderCompleted {
        kind: ReminderKind,
        id: String,
    },
    PomodoroStarted,
    PomodoroPaused,
    PomodoroCompleted,
    TodoCompleted {
        id: String,
    },
    SedentaryWarning,
    UserActivityResumed,
    DisplayChanged,
    Tick {
        now_ms: u64,
    },
}

#[derive(Debug, Default)]
pub struct EventNormalizer {
    near: bool,
    dragging: bool,
}

impl EventNormalizer {
    pub fn normalize(&mut self, event: PetEvent) -> Option<PetEvent> {
        match event {
            PetEvent::PointerNear { .. } if self.near => None,
            PetEvent::PointerNear { .. } => {
                self.near = true;
                Some(event)
            }
            PetEvent::PointerExited => {
                self.near = false;
                Some(event)
            }
            PetEvent::DragStarted { .. } => {
                self.dragging = true;
                Some(event)
            }
            PetEvent::DragReleased { .. } => {
                self.dragging = false;
                Some(event)
            }
            PetEvent::PetClicked { .. } if self.dragging => None,
            _ => Some(event),
        }
    }
}

pub fn hit_region(y: f32, height: f32) -> HitRegion {
    if y <= height * 0.42 {
        HitRegion::Head
    } else {
        HitRegion::Body
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_event_normalizer_deduplicates_pointer_near() {
        let mut n = EventNormalizer::default();
        assert!(
            n.normalize(PetEvent::PointerNear { distance_px: 30. })
                .is_some()
        );
        assert!(
            n.normalize(PetEvent::PointerNear { distance_px: 20. })
                .is_none()
        );
        n.normalize(PetEvent::PointerExited);
        assert!(
            n.normalize(PetEvent::PointerNear { distance_px: 20. })
                .is_some()
        );
    }
    #[test]
    fn test_click_region_head_and_body_are_distinct() {
        assert_eq!(hit_region(20., 100.), HitRegion::Head);
        assert_eq!(hit_region(80., 100.), HitRegion::Body);
    }
    #[test]
    fn test_drag_release_does_not_emit_click() {
        let mut n = EventNormalizer::default();
        n.normalize(PetEvent::DragStarted {
            pointer_x: 0.,
            pointer_y: 0.,
        });
        assert!(
            n.normalize(PetEvent::PetClicked {
                region: HitRegion::Body,
                click_count: 1
            })
            .is_none()
        );
    }
}
