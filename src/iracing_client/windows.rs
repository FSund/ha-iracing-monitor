use crate::iracing_client::SimClient;  // Make sure to import the trait
use simetry::iracing;
use std::time::Duration;
use tokio::time::timeout;

pub struct IracingClient {
    client: Option<iracing::Client>,
}

impl IracingClient {
    pub async fn new() -> Self {
        Self { client: None }
    }

    fn is_connected(&self) -> bool {
        self.client.is_some()
    }
}

#[async_trait::async_trait]
impl SimClient for IracingClient {
    async fn connect(&mut self) -> bool {
        if !self.is_connected() {
            log::debug!("Waiting for iRacing connection...");
            let connect_result = timeout(
                Duration::from_secs(5),
                iracing::Client::try_connect()
            ).await;
            
            self.client = match connect_result {
                Ok(client_result) => client_result.ok(),
                Err(_elapsed) => {
                    log::debug!("Connection attempt timed out.");
                    None
                }
            };
        }
        self.is_connected()
    }

    async fn get_current_session_type(&mut self) -> Option<String> {
        if !self.connect().await {
            return None;
        }
    
        let client = self.client.as_mut().expect("Could not get client as mut");
        let sim_state = match client.next_sim_state().await {
            Some(state) => state,
            None => {
                // iRacing most likely disconnected, reset client
                log::info!("Lost connection to iRacing.");
                self.client = None;
                return None;
            }
        };
        let session_info = sim_state.session_info();
        let session_num = sim_state.read_name::<i32>("SessionNum")?;
        
        let sessions = session_info["SessionInfo"]["Sessions"].as_vec()?;
        
        sessions.iter()
            .find(|session| session["SessionNum"].as_i64().is_some_and(|num| num as i32 == session_num))
            .and_then(|session| session["SessionType"].as_str())
            .map(String::from)
    }
}