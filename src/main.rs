
use anyhow::{Context, Result};
use chrono::Utc;
// use iracing::{Client, Session, SessionFlags, SessionState};
// use simetry::iracing::Client;
use simetry::iracing;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::Serialize;
use std::time::Duration;
use tokio::time;
use yaml_rust::Yaml;

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
            iracing: IracingClient::new().await,
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

    // fn get_session_type(session: &Session) -> String {
    //     match session.session_type() {
    //         iracing::SessionType::Practice => "Practice",
    //         iracing::SessionType::OpenQualify => "OpenQualify",
    //         iracing::SessionType::LoneQualify => "LoneQualify",
    //         iracing::SessionType::Race => "Race",
    //         _ => "Unknown"
    //     }.to_string()
    // }

    async fn get_current_state(&mut self) -> MonitorState {
        let connected = self.iracing.is_connected();
        let session_type = self.iracing.get_current_session_type().await.unwrap_or("None".to_string());
        MonitorState {
            connected,
            session_type,
            timestamp: Utc::now().to_rfc3339(),
        }
    }

    async fn run(&mut self) -> Result<()> {
        println!("Starting iRacing monitor...");
        
        let mut interval = time::interval(Duration::from_secs(5));
        
        loop {
            interval.tick().await;
            
            let state = self.get_current_state().await;
            if let Err(e) = self.publish_state(&state).await {
                eprintln!("Failed to publish state: {}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut monitor = Monitor::new("YOUR_HOME_ASSISTANT_IP", 1883).await?;
    monitor.run().await
}