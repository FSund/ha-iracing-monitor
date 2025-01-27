mod iracing_client;
mod frontend;

use iracing_client::SimClient;
use anyhow::{Context, Result};
use chrono::Utc;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::Serialize;
use std::time::Duration;
use tokio::time;
use env_logger::{Builder, Target};
use log::LevelFilter;
use iced;
use frontend::IracingMonitorGui;


#[derive(Debug, Serialize, Clone, PartialEq)]
struct MonitorState {
    connected: bool,
    // in_session: bool,
    current_session_type: String,
    // session_state: String,
    timestamp: String,
}

impl Default for MonitorState {
    fn default() -> Self {
        Self {
            connected: false,
            // in_session: false,
            current_session_type: "None".to_string(), // "None" registers as "Unknown" in Home Assistant
            // session_state: "None".to_string(),
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

struct Monitor {
    iracing: iracing_client::Client,
    mqtt: Option<AsyncClient>,
    last_state: Option<MonitorState>,
    mqtt_topic: String,
}

impl Monitor {
    async fn new(mqtt_client: Option<AsyncClient>) -> Result<Self> {
        Ok(Self {
            iracing: iracing_client::Client::new().await,
            mqtt: mqtt_client,
            last_state: None,
            mqtt_topic: "homeassistant/sensor/iracing/state".to_string(),
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
        // let connected = self.iracing.is_connected();
        match self.iracing.get_current_session_type().await {
            Some(session_type) => {
                log::debug!("Found session_type: {}", session_type);
                MonitorState {
                    connected: true,
                    current_session_type: session_type,
                    timestamp: Utc::now().to_rfc3339(),
                }
            },
            None => {
                MonitorState {
                    connected: false,
                    current_session_type: "Disconnected".to_string(),
                    timestamp: Utc::now().to_rfc3339(),
                }
            }
        }
    }

    async fn run(&mut self) -> Result<()> {
        log::info!("Starting iRacing monitor...");

        if let Some(mqtt) = self.mqtt.as_mut() {
            if register_device(mqtt).await.is_err() {
                log::warn!("Failed to register MQTT device");
            }
        }

        // let initial_state = MonitorState{
        //     current_session_type: "Disconnected".to_string(),
        //     ..Default::default()
        // };
        // if let Err(e) = self.publish_state(&initial_state).await {
        //     log::warn!("Failed to publish state: {}", e);
        // }

        log::info!("Waiting for connection to iRacing.");
        if self.iracing.connect().await {
            log::info!("Connected to iRacing!");
        } else {
            log::info!("Failed to connect to iRacing.");
        }

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

async fn register_device(mqtt: &mut AsyncClient) -> Result<()> {
    // homeassistant/sensor/hp_1231232/config
    // <discovery_prefix>/<component>/[<node_id>/]<object_id>/config
    // Best practice for entities with a unique_id is to set <object_id> to unique_id and omit the <node_id>.

    let configuration_topic = "homeassistant/sensor/iracing/config";
    let config = serde_json::json!({
        "name": "Session type",
        "state_topic": "homeassistant/sensor/iracing/state",
        // "device_class": "timestamp",  // optional, will use "None: Generic sensor. (This is the default and doesn’t need to be set.)" if not set
        "value_template": "{{ value_json.current_session_type }}",
        "unique_id": "iracing_session_type",
        "expire_after": 30,
        "device": {
            "identifiers": "my_unique_id",
            "name": "iRacing Simulator",
        }
    });

    mqtt.publish(
        configuration_topic,
        QoS::AtLeastOnce,
        true, // retain flag set to true for discovery
        serde_json::to_string(&config)?,
    )
    .await
    .context("Failed to publish MQTT discovery configuration")?;

    log::info!("Registered device with Home Assistant.");
    Ok(())
}

// #[tokio::main]
// async fn main() -> Result<()> {
//     // env_logger::init();
//     let mut builder = Builder::from_default_env();
    
//     // Set external crates to INFO level
//     // builder.filter_module("rumqttc", LevelFilter::Info);
    
//     // Keep your application at DEBUG level
//     builder.filter_module("iracing_ha_monitor", LevelFilter::Debug);
    
//     // Apply the configuration
//     builder.target(Target::Stdout)
//            .init();

//     log::info!("Welcome to iRacing HA monitor!");

//     let mqtt_host = std::env::var("MQTT_HOST").ok();
//     let mqtt_port = std::env::var("MQTT_PORT")
//         .ok()
//         .and_then(|p| p.parse().ok())
//         .or(Some(1883));

//     let mqtt_user = std::env::var("MQTT_USER").unwrap_or("".to_string());
//     let mqtt_password = std::env::var("MQTT_PASSWORD").unwrap_or("".to_string());

//     // Set up MQTT client
//     let mqtt_client = if let (Some(host), Some(port)) = (mqtt_host, mqtt_port) {
//         let mut mqtt_options = MqttOptions::new("iracing-monitor", host, port);
//         mqtt_options.set_keep_alive(Duration::from_secs(5));
//         mqtt_options.set_credentials(mqtt_user, mqtt_password);
//         let (mqtt_client, mut mqtt_eventloop) = AsyncClient::new(mqtt_options, 10);

//         // Start MQTT event loop
//         tokio::spawn(async move {
//             while let Ok(_notification) = mqtt_eventloop.poll().await {
//                 // Handle MQTT events if needed
//             }
//         });
//         log::info!("MQTT client set up.");
//         Some(mqtt_client)
//     } else {
//         log::info!("Missing MQTT config, skipping MQTT publishing.");
//         None
//     };
    
//     let mut monitor = Monitor::new(mqtt_client).await?;
//     monitor.run().await
// }


pub fn main() -> iced::Result {
    iced::application("IRacingMonitor - Iced", IracingMonitorGui::update, IracingMonitorGui::view)
        .run_with(IracingMonitorGui::new)
}