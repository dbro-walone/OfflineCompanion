use crate::platform::WorkArea;
pub const THROW_THRESHOLD_PX_S: f32 = 900.0;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleasePath {
    Drop,
    Thrown,
    Edge,
}
pub fn release_path(vx: f32, vy: f32, x: i32, width: u32, area: WorkArea) -> ReleasePath {
    if x <= area.left + 8 || x + width as i32 >= area.right - 8 {
        ReleasePath::Edge
    } else if vx.hypot(vy) >= THROW_THRESHOLD_PX_S {
        ReleasePath::Thrown
    } else {
        ReleasePath::Drop
    }
}
pub fn clamp_to_work_area(x: i32, y: i32, width: u32, height: u32, area: WorkArea) -> (i32, i32) {
    (
        x.clamp(area.left, area.right - width as i32),
        y.clamp(area.top, area.bottom - height as i32),
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
    fn test_work_area_clamps_negative_monitor_position() {
        assert_eq!(
            clamp_to_work_area(-3000, -500, 200, 200, area()),
            (-1920, -100)
        );
    }
}
