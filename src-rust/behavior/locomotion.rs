use crate::platform::WorkArea;

pub const THROW_THRESHOLD_PX_S: f32 = 900.0;
pub const MAX_THROW_SPEED_PX_S: f32 = 2_400.0;
pub const MAX_FLIGHT_DISPLACEMENT_PX: f32 = 1_200.0;
const EDGE_THRESHOLD_PX: i32 = 8;
const GRAVITY_PX_S2: f32 = 1_800.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleasePath {
    Drop,
    Thrown,
    EdgeLeft,
    EdgeRight,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReleasePlan {
    pub path: ReleasePath,
    pub velocity_x: f32,
    pub velocity_y: f32,
    pub animate_flight: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct MotionState {
    pub x: f32,
    pub y: f32,
    pub velocity_x: f32,
    pub velocity_y: f32,
    origin_x: f32,
    origin_y: f32,
    area: WorkArea,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionStep {
    pub x: i32,
    pub y: i32,
    pub landed: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct LocomotionController {
    pub reduce_motion: bool,
}

impl LocomotionController {
    pub fn release(
        self,
        velocity_x: f32,
        velocity_y: f32,
        x: i32,
        width: u32,
        area: WorkArea,
    ) -> ReleasePlan {
        let path = release_path(velocity_x, velocity_y, x, width, area);
        let speed = velocity_x.hypot(velocity_y);
        let scale = if speed > MAX_THROW_SPEED_PX_S {
            MAX_THROW_SPEED_PX_S / speed
        } else {
            1.0
        };
        ReleasePlan {
            path,
            velocity_x: if path == ReleasePath::Thrown {
                velocity_x * scale
            } else {
                0.0
            },
            velocity_y: if path == ReleasePath::Thrown {
                velocity_y * scale
            } else {
                80.0
            },
            animate_flight: !self.reduce_motion
                && matches!(path, ReleasePath::Drop | ReleasePath::Thrown),
        }
    }
}

impl MotionState {
    pub fn new(x: i32, y: i32, plan: ReleasePlan, area: WorkArea, width: u32, height: u32) -> Self {
        Self {
            x: x as f32,
            y: y as f32,
            velocity_x: plan.velocity_x,
            velocity_y: plan.velocity_y,
            origin_x: x as f32,
            origin_y: y as f32,
            area,
            width,
            height,
        }
    }

    pub fn step(&mut self, elapsed_seconds: f32) -> MotionStep {
        let elapsed = elapsed_seconds.clamp(0.001, 0.05);
        self.velocity_y = (self.velocity_y + GRAVITY_PX_S2 * elapsed)
            .clamp(-MAX_THROW_SPEED_PX_S, MAX_THROW_SPEED_PX_S);
        self.velocity_x = self
            .velocity_x
            .clamp(-MAX_THROW_SPEED_PX_S, MAX_THROW_SPEED_PX_S);
        self.x += self.velocity_x * elapsed;
        self.y += self.velocity_y * elapsed;
        self.velocity_x *= 0.985;
        self.x = self.x.clamp(
            self.origin_x - MAX_FLIGHT_DISPLACEMENT_PX,
            self.origin_x + MAX_FLIGHT_DISPLACEMENT_PX,
        );
        self.y = self.y.clamp(
            self.origin_y - MAX_FLIGHT_DISPLACEMENT_PX,
            self.origin_y + MAX_FLIGHT_DISPLACEMENT_PX,
        );
        let (x, y) = clamp_to_work_area(
            self.x.round() as i32,
            self.y.round() as i32,
            self.width,
            self.height,
            self.area,
        );
        let landed = y >= self.area.bottom - self.height as i32;
        MotionStep { x, y, landed }
    }
}

pub fn release_path(vx: f32, vy: f32, x: i32, width: u32, area: WorkArea) -> ReleasePath {
    if x <= area.left + EDGE_THRESHOLD_PX {
        ReleasePath::EdgeLeft
    } else if x + width as i32 >= area.right - EDGE_THRESHOLD_PX {
        ReleasePath::EdgeRight
    } else if vx.hypot(vy) >= THROW_THRESHOLD_PX_S {
        ReleasePath::Thrown
    } else {
        ReleasePath::Drop
    }
}

pub fn clamp_to_work_area(x: i32, y: i32, width: u32, height: u32, area: WorkArea) -> (i32, i32) {
    (
        x.clamp(area.left, (area.right - width as i32).max(area.left)),
        y.clamp(area.top, (area.bottom - height as i32).max(area.top)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> WorkArea {
        WorkArea {
            left: -1920,
            top: -100,
            right: 0,
            bottom: 980,
        }
    }

    #[test]
    fn test_throw_threshold_selects_thrown_path() {
        assert_eq!(
            release_path(901., 0., -1000, 200, area()),
            ReleasePath::Thrown
        );
    }

    #[test]
    fn test_left_edge_selects_edge_left() {
        assert_eq!(
            release_path(0., 0., -1920, 200, area()),
            ReleasePath::EdgeLeft
        );
    }

    #[test]
    fn test_right_edge_selects_edge_right() {
        assert_eq!(
            release_path(0., 0., -200, 200, area()),
            ReleasePath::EdgeRight
        );
    }

    #[test]
    fn test_reduce_motion_keeps_release_semantics_without_flight() {
        let plan = LocomotionController {
            reduce_motion: true,
        }
        .release(1_000., 0., -1000, 200, area());
        assert_eq!(plan.path, ReleasePath::Thrown);
        assert!(!plan.animate_flight);
    }

    #[test]
    fn thrown_speed_and_displacement_are_bounded() {
        let plan = LocomotionController {
            reduce_motion: false,
        }
        .release(10_000., -10_000., -1000, 200, area());
        assert!(plan.velocity_x.hypot(plan.velocity_y) <= MAX_THROW_SPEED_PX_S + 0.1);
        let mut motion = MotionState::new(-1000, 0, plan, area(), 200, 200);
        for _ in 0..10_000 {
            let step = motion.step(0.05);
            assert!((-1920..=-200).contains(&step.x));
            assert!((-100..=780).contains(&step.y));
            if step.landed {
                return;
            }
        }
        panic!("bounded throw should eventually land");
    }

    #[test]
    fn test_work_area_clamps_negative_monitor_position() {
        assert_eq!(
            clamp_to_work_area(-3000, -500, 200, 200, area()),
            (-1920, -100)
        );
    }
}
