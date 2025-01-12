
use anyhow::{Context, Result};
use chrono::Utc;
// use iracing::{Client, Session, SessionFlags, SessionState};
// use simetry::iracing::Client;
use simetry::iracing;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::Serialize;
use std::time::Duration;
use tokio::time;

#[derive(Debug, Serialize, Clone, PartialEq)]
struct MonitorState {
    connected: bool,
    // in_session: bool,
    session_type: String,
    // session_state: String,
    timestamp: String,
}

impl Default for MonitorState {
    fn default() -> Self {
        Self {
            connected: false,
            // in_session: false,
            session_type: "None".to_string(),
            // session_state: "None".to_string(),
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

struct IracingClient {
    client: Option<iracing::Client>,
}

impl IracingClient {
    async fn new() -> Self {
        Self {
            client: iracing::Client::try_connect().await.ok()
        }
    }

    fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    async fn connect(&mut self) -> bool {
        if !self.is_connected() {
            self.client = iracing::Client::try_connect().await.ok()
        }
        self.is_connected()
    }

    async fn get_current_session_type(&mut self) -> Option<String> {
        if !self.connect().await {
            return None;
        }
    
        let client = self.client.as_mut()?;
        let sim_state = client.next_sim_state().await?;
        let session_info = sim_state.session_info();
        let session_num = sim_state.read_name::<i32>("SessionNum")?;
        
        let sessions = session_info["SessionInfo"]["Sessions"].as_vec()?;
        
        sessions.iter()
            .find(|session| session["SessionNum"].as_i64().is_some_and(|num| num as i32 == session_num))
            .and_then(|session| session["SessionType"].as_str())
            .map(String::from)
    }
}

struct Monitor {
    iracing: IracingClient,
    mqtt: Option<AsyncClient>,
    last_state: Option<MonitorState>,
    mqtt_topic: String,
}

impl Monitor {
    async fn new(mqtt_client: Option<AsyncClient>) -> Result<Self> {
        Ok(Self {
            iracing: IracingClient::new().await,
            mqtt: mqtt_client,
            last_state: None,
            mqtt_topic: "iracing/status".to_string(),
        })
    }

    async fn publish_state(&mut self, state: &MonitorState) -> Result<()> {
        if Some(state) != self.last_state.as_ref() {
            if let Some(mqtt) = self.mqtt.as_mut() {
                let payload = serde_json::to_string(&state)?;
                mqtt
                    .publish(&self.mqtt_topic, QoS::AtLeastOnce, false, payload)
                    .await
                    .context("Failed to publish MQTT message")?;
                
                log::debug!("Published state: {:?}", state);
            }
            self.last_state = Some(state.clone());
        }
        Ok(())
    }

    async fn get_current_state(&mut self) -> MonitorState {
        let connected = self.iracing.is_connected();
        let session_type = self.iracing.get_current_session_type().await.unwrap_or("None".to_string());
        log::debug!("Found session_type: {}", session_type);
        MonitorState {
            connected,
            session_type,
            timestamp: Utc::now().to_rfc3339(),
        }
    }

    async fn run(&mut self) -> Result<()> {
        log::info!("Starting iRacing monitor...");
        
        let mut interval = time::interval(Duration::from_secs(5));
        
        loop {
            interval.tick().await;
            
            let state = self.get_current_state().await;
            if let Err(e) = self.publish_state(&state).await {
                log::warn!("Failed to publish state: {}", e);
            }

            log::debug!("TICK");
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    log::debug!("This is a debug message");

    let mqtt_host = std::env::var("MQTT_HOST").ok();
    let mqtt_port = std::env::var("MQTT_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .or(Some(1883));

    let mqtt_user = std::env::var("MQTT_USER").unwrap_or("".to_string());
    let mqtt_password = std::env::var("MQTT_PASSWORD").unwrap_or("".to_string());

    // Set up MQTT client
    let mqtt_client = if let (Some(host), Some(port)) = (mqtt_host, mqtt_port) {
        let mut mqtt_options = MqttOptions::new("iracing-monitor", host, port);
        mqtt_options.set_keep_alive(Duration::from_secs(5));
        mqtt_options.set_credentials(mqtt_user, mqtt_password);
        let (mqtt_client, mut mqtt_eventloop) = AsyncClient::new(mqtt_options, 10);

        // Start MQTT event loop
        tokio::spawn(async move {
            while let Ok(_notification) = mqtt_eventloop.poll().await {
                // Handle MQTT events if needed
            }
        });

        Some(mqtt_client)
    } else {
        None
    };
    
    let mut monitor = Monitor::new(mqtt_client).await?;
    monitor.run().await
}