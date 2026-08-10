use std::path::PathBuf;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderState {
    pub atlas: PathBuf,
    pub frame_index: u32,
    pub frame_width: u32,
    pub frame_height: u32,
    pub columns: u32,
    pub mirror_x: bool,
}
