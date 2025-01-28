use crate::iracing_client::SimClient;

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

    async fn get_current_session_type(&mut self) -> Option<String> {
        if !self.connect().await {
            return None;
        }

        if self.connected {
            Some("Practice".to_string()) // Mock implementation
        } else {
            None
        }
    }
}
