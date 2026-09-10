pub use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct SessionState {
    /// Seconds left in the current session, `None` for untimed sessions.
    pub time_remaining: Option<f64>,
    /// Full session info document as JSON, `None` when unchanged since the last poll.
    pub session_info: Option<serde_json::Value>,
}

#[async_trait]
pub trait SimClient {
    fn new() -> Self;
    // async fn connect(&mut self) -> bool;
    // fn is_connected(&self) -> bool;
    async fn get_current_session_state(&mut self) -> Option<SessionState>;
}

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::IracingClient as Client;

#[cfg(not(target_os = "windows"))]
mod mock;
#[cfg(not(target_os = "windows"))]
pub use mock::MockClient as Client;
