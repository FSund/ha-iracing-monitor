use crate::iracing_client::SimClient;

pub struct MockClient {
    connected: bool,
}

impl MockClient {
    pub async fn new() -> Self {
        Self { connected: false }
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

#[async_trait::async_trait]
impl SimClient for MockClient {
    async fn connect(&mut self) -> bool {
        self.connected = true;
        self.connected
    }

    async fn get_current_session_type(&mut self) -> Option<String> {
        if self.connected {
            Some("Practice".to_string()) // Mock implementation
        } else {
            None
        }
    }
}