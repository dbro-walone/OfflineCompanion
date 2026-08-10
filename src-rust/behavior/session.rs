#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEnd {
    Accepted,
    PointerLeft,
    TimedOut,
}
#[derive(Debug, Clone)]
pub struct InteractionSession {
    deadline_ms: u64,
    ended: Option<SessionEnd>,
}
impl InteractionSession {
    pub fn new(now_ms: u64) -> Self {
        Self {
            deadline_ms: now_ms + 4000,
            ended: None,
        }
    }
    pub fn click(&mut self) {
        self.ended = Some(SessionEnd::Accepted)
    }
    pub fn pointer_left(&mut self) {
        self.ended = Some(SessionEnd::PointerLeft)
    }
    pub fn tick(&mut self, now_ms: u64) {
        if self.ended.is_none() && now_ms >= self.deadline_ms {
            self.ended = Some(SessionEnd::TimedOut)
        }
    }
    pub fn end(&self) -> Option<SessionEnd> {
        self.ended
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_waiting_session_accepts_click() {
        let mut s = InteractionSession::new(0);
        s.click();
        assert_eq!(s.end(), Some(SessionEnd::Accepted));
    }
    #[test]
    fn test_waiting_session_exits_on_pointer_leave() {
        let mut s = InteractionSession::new(0);
        s.pointer_left();
        assert_eq!(s.end(), Some(SessionEnd::PointerLeft));
    }
    #[test]
    fn test_waiting_session_times_out() {
        let mut s = InteractionSession::new(0);
        s.tick(4000);
        assert_eq!(s.end(), Some(SessionEnd::TimedOut));
    }
}
