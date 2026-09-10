pub use async_trait::async_trait;
use std::collections::BTreeMap;

use crate::config::TelemetrySensor;

#[derive(Debug, Clone)]
pub struct SessionState {
    /// Values for the configured telemetry sensors, keyed by sensor id.
    /// Unreadable variables are `null`.
    pub telemetry: serde_json::Map<String, serde_json::Value>,
    /// Full session info document as JSON, `None` when unchanged since the last poll.
    pub session_info: Option<serde_json::Value>,
}

#[async_trait]
pub trait SimClient {
    fn new() -> Self;
    async fn get_current_session_state(
        &mut self,
        telemetry: &BTreeMap<String, TelemetrySensor>,
    ) -> Option<SessionState>;
}

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::IracingClient as Client;

#[cfg(not(target_os = "windows"))]
mod mock;
#[cfg(not(target_os = "windows"))]
pub use mock::MockClient as Client;
