
use anyhow::{Context, Result};
use chrono::Utc;
// use iracing::{Client, Session, SessionFlags, SessionState};
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::Serialize;
use std::time::Duration;
use tokio::time;

#[derive(Debug, Serialize, Clone, PartialEq)]
struct MonitorState {
    connected: bool,
    in_session: bool,
    session_type: String,
    session_state: String,
    timestamp: String,
}

impl Default for MonitorState {
    fn default() -> Self {
        Self {
            connected: false,
            in_session: false,
            session_type: "None".to_string(),
            session_state: "None".to_string(),
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

struct Monitor {
    iracing: Client,
    mqtt: AsyncClient,
    last_state: Option<MonitorState>,
    mqtt_topic: String,
}

impl Monitor {
    async fn new(mqtt_host: &str, mqtt_port: u16) -> Result<Self> {
        // Set up MQTT client
        let mut mqtt_options = MqttOptions::new("iracing-monitor", mqtt_host, mqtt_port);
        mqtt_options.set_keep_alive(Duration::from_secs(5));
        let (mqtt_client, mut mqtt_eventloop) = AsyncClient::new(mqtt_options, 10);
        
        // Start MQTT event loop
        tokio::spawn(async move {
            while let Ok(_notification) = mqtt_eventloop.poll().await {
                // Handle MQTT events if needed
            }
        });

        Ok(Self {
            iracing: Client::new()?,
            mqtt: mqtt_client,
            last_state: None,
            mqtt_topic: "iracing/status".to_string(),
        })
    }

    async fn publish_state(&mut self, state: &MonitorState) -> Result<()> {
        if Some(state) != self.last_state.as_ref() {
            let payload = serde_json::to_string(&state)?;
            self.mqtt
                .publish(&self.mqtt_topic, QoS::AtLeastOnce, false, payload)
                .await
                .context("Failed to publish MQTT message")?;
            
            println!("Published state: {:?}", state);
            self.last_state = Some(state.clone());
        }
        Ok(())
    }

    fn get_session_type(session: &Session) -> String {
        match session.session_type() {
            iracing::SessionType::Practice => "Practice",
            iracing::SessionType::OpenQualify => "OpenQualify",
            iracing::SessionType::LoneQualify => "LoneQualify",
            iracing::SessionType::Race => "Race",
            _ => "Unknown"
        }.to_string()
    }

    fn get_current_state(&mut self) -> Result<MonitorState> {
        // Try to connect/reconnect to iRacing
        if !self.iracing.is_connected() {
            self.iracing.connect()?;
        }

        // If still not connected after attempt, return disconnected state
        if !self.iracing.is_connected() {
            return Ok(MonitorState::default());
        }

        // Get current session
        let session = self.iracing.session()?;
        
        let session_type = Self::get_session_type(&session);
        
        let session_state = match session.state() {
            SessionState::Invalid => "Invalid",
            SessionState::GetInCar => "GetInCar",
            SessionState::Warmup => "Warmup",
            SessionState::ParadeLaps => "ParadeLaps",
            SessionState::Racing => "Racing",
            SessionState::Checkered => "Checkered",
            SessionState::CoolDown => "CoolDown",
        };

        Ok(MonitorState {
            connected: true,
            in_session: session_state != "Invalid",
            session_type,
            session_state: session_state.to_string(),
            timestamp: Utc::now().to_rfc3339(),
        })
    }

    async fn run(&mut self) -> Result<()> {
        println!("Starting iRacing monitor...");
        
        let mut interval = time::interval(Duration::from_secs(1));
        
        loop {
            interval.tick().await;
            
            match self.get_current_state() {
                Ok(state) => {
                    if let Err(e) = self.publish_state(&state).await {
                        eprintln!("Failed to publish state: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to get iRacing state: {}", e);
                    // Publish disconnected state
                    self.publish_state(&MonitorState::default()).await?;
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut monitor = Monitor::new("YOUR_HOME_ASSISTANT_IP", 1883).await?;
    monitor.run().await
}