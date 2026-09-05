use crate::iracing_client::{SessionState, SimClient};

pub struct MockClient {
    connected: bool,
}

impl MockClient {
    // fn is_connected(&self) -> bool {
    //     self.connected
    // }

    async fn connect(&mut self) -> bool {
        self.connected = true;
        self.connected
    }
}

#[async_trait::async_trait]
impl SimClient for MockClient {
    fn new() -> Self {
        Self { connected: false }
    }

    async fn get_current_session_state(&mut self) -> Option<SessionState> {
        if !self.connect().await {
            return None;
        }

        if self.connected {
            Some(SessionState {
                session_type: "Practice".to_string(), // Mock implementation
                time_remaining: Some(1800.0),
            })
        } else {
            None
        }
    }
}
