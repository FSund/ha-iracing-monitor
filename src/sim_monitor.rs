use crate::config;
use crate::config::AppConfig;
use crate::iracing_client;

use anyhow::{Context, Result};
use chrono::{SecondsFormat, TimeDelta, Utc};
use futures::channel::mpsc;
use futures::prelude::sink::SinkExt;
use futures::prelude::stream::StreamExt;
use futures::stream::Stream;
use iced_futures::stream as iced_stream;
use iracing_client::SimClient;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::time::Duration;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

/// All data topics live under this prefix: `iracing/state` and `iracing/<group>`.
const TOPIC_PREFIX: &str = "iracing";
const STATE_TOPIC: &str = "iracing/state";
/// Device-based discovery: one payload describing all sensors.
const DISCOVERY_TOPIC: &str = "homeassistant/device/iracing/config";
/// Where the current session type lives in the session info document.
const SESSION_TYPE_PATH: &str = "SessionInfo.Sessions[{CurrentSessionNum}].SessionType";

#[derive(Debug, Serialize, Clone, PartialEq, EnumIter)]
pub enum SessionType {
    // Unknown,
    Disconnected,
    Practice,
    Qualify,
    Race,
    #[serde(rename = "Lone Qualify")]
    LoneQualify,
    #[serde(rename = "Offline Testing")]
    OfflineTesting,
}

impl Display for SessionType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        // Use serde serialization to ensure Display matches JSON output
        let json_value = serde_json::to_value(self).unwrap();
        write!(f, "{}", json_value.as_str().unwrap())
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct SimMonitorState {
    pub connected: bool,
    // in_session: bool,
    pub current_session_type: SessionType,
    // session_state: String,
    /// Seconds left in the current session, `None` for untimed sessions.
    pub session_time_remaining: Option<i64>,
    /// RFC3339 timestamp of when the current session is scheduled to end.
    pub session_end_time: Option<String>,
    pub timestamp: String,
}

impl Default for SimMonitorState {
    fn default() -> Self {
        Self {
            connected: false,
            // in_session: false,
            current_session_type: SessionType::Disconnected,
            // session_state: "None".to_string(),
            session_time_remaining: None,
            session_end_time: None,
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 1883,
            user: "".to_string(),
            password: "".to_string(),
        }
    }
}

pub struct SimMonitor {
    iracing: iracing_client::Client,
    mqtt: Option<AsyncClient>,
    last_state: Option<SimMonitorState>,
    attributes_config: BTreeMap<String, BTreeMap<String, String>>,
    /// Latest full session info document, kept for re-projection on config changes.
    session_info: Option<serde_json::Value>,
    /// Last published payload per attribute group, used to skip redundant publishes.
    last_attributes: BTreeMap<String, serde_json::Map<String, serde_json::Value>>,

    mqtt_eventloop_handle: Option<tokio::task::JoinHandle<()>>,
    mqtt_eventloop: Option<rumqttc::EventLoop>,
}

impl SimMonitor {
    pub fn new(
        mqtt_config: Option<MqttConfig>,
        attributes_config: BTreeMap<String, BTreeMap<String, String>>,
    ) -> Self {
        let mut monitor = Self {
            iracing: iracing_client::Client::new(),
            mqtt: None,
            last_state: None,
            attributes_config,
            session_info: None,
            last_attributes: BTreeMap::new(),
            mqtt_eventloop_handle: None,
            mqtt_eventloop: None,
        };
        monitor.set_mqtt_config(mqtt_config);
        monitor
    }

    fn set_mqtt_config(&mut self, mqtt_config: Option<MqttConfig>) {
        // If we have an existing event loop, abort it before creating a new one
        if let Some(handle) = self.mqtt_eventloop_handle.take() {
            log::debug!("Aborting MQTT event loop");
            handle.abort();
        }

        // Force a fresh publish of all attribute groups on the (re)configured connection
        self.last_attributes.clear();

        let Some(mqtt_config) = mqtt_config else {
            log::debug!("Disabling MQTT");
            self.mqtt = None;
            return;
        };

        let mut mqtt_options =
            MqttOptions::new("iracing-monitor", mqtt_config.host, mqtt_config.port);
        mqtt_options.set_keep_alive(Duration::from_secs(5));
        mqtt_options.set_credentials(mqtt_config.user, mqtt_config.password);
        let (mqtt_client, mqtt_eventloop) = AsyncClient::new(mqtt_options, 10);

        // Store the client and event loop
        self.mqtt = Some(mqtt_client);
        self.mqtt_eventloop = Some(mqtt_eventloop); // Store the event loop without starting it yet
    }

    async fn start_mqtt_eventloop(&mut self) {
        if self.mqtt.is_none() {
            log::debug!("MQTT disabled, skipping event loop start");
            return;
        }

        if let Some(mut mqtt_eventloop) = self.mqtt_eventloop.take() {
            // Spawn and store the event loop handle
            log::debug!("Starting MQTT event loop");
            self.mqtt_eventloop_handle = Some(tokio::spawn(async move {
                loop {
                    match mqtt_eventloop.poll().await {
                        Ok(_notification) => {
                            // log::debug!("MQTT event: {:?}", notification);
                        }
                        Err(e) => {
                            // Just log the error but keep polling - the event loop will handle reconnection
                            // log::error!("MQTT error (will retry automatically): {:?}", e);
                            log::error!("MQTT error {e}");
                            tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;
                        }
                    }

                    // Small yield to prevent tight loop
                    // tokio::task::yield_now().await;

                    // TODO: is this the way?
                    // tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }));

            log::debug!("MQTT client set up.");

            // Register the device
            let attribute_groups: Vec<String> = self.attributes_config.keys().cloned().collect();
            if let Some(mqtt) = self.mqtt.as_mut() {
                if let Err(e) = register_device(mqtt, &attribute_groups).await {
                    log::warn!("Failed to register MQTT device ({e})");
                }
                // Add a small delay to ensure registration is processed
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        } else {
            log::error!("Failed to start MQTT event loop, missing event loop");
        }
    }

    async fn publish_state(&mut self, state: &SimMonitorState) -> Result<()> {
        if Some(state) != self.last_state.as_ref() {
            if let Some(mqtt) = self.mqtt.as_mut() {
                let payload = serde_json::to_string(&state)?;
                log::debug!(
                    "Attempting to publish to topic: {} with payload: {}",
                    STATE_TOPIC,
                    &payload
                );

                // Spawn MQTT publish in separate task
                let mqtt_clone = mqtt.clone();
                let state_clone = state.clone();
                tokio::spawn(async move {
                    match tokio::time::timeout(
                        Duration::from_secs(5), // 5 second timeout
                        mqtt_clone.publish(STATE_TOPIC, QoS::AtLeastOnce, false, payload),
                    )
                    .await
                    {
                        Ok(result) => match result {
                            Ok(_) => {
                                // this isn't really true, it just means the connection
                                // hasn't timed out yet
                                log::debug!("Payload delivered to MQTT event loop");
                            }
                            Err(e) => {
                                log::warn!("Failed to publish state via MQTT: {}", e);
                            }
                        },
                        Err(_) => {
                            log::warn!("MQTT publish operation timed out after 5 seconds");
                        }
                    }
                });
                self.last_state = Some(state_clone);
            } else {
                log::debug!("Unable to publish state to MQTT, missing MQTT config");
            }
        }
        self.publish_attributes().await;
        Ok(())
    }

    /// Projects the session info document into the configured attribute groups and
    /// publishes each changed group as retained JSON to `iracing/<group>`.
    async fn publish_attributes(&mut self) {
        let Some(mqtt) = self.mqtt.clone() else {
            return;
        };
        let Some(info) = self.session_info.as_ref() else {
            return;
        };

        for (group, fields) in &self.attributes_config {
            let payload: serde_json::Map<String, serde_json::Value> = fields
                .iter()
                .filter_map(|(name, path)| {
                    resolve_path(info, path).map(|value| (name.clone(), value))
                })
                .collect();

            if self.last_attributes.get(group) == Some(&payload) {
                continue;
            }

            let mut full = payload.clone();
            full.insert(
                "timestamp".to_string(),
                Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true).into(),
            );
            let topic = format!("{TOPIC_PREFIX}/{group}");
            let json = serde_json::Value::Object(full).to_string();
            log::debug!("Publishing attributes to {topic}: {json}");
            match mqtt.publish(&topic, QoS::AtLeastOnce, true, json).await {
                Ok(()) => {
                    self.last_attributes.insert(group.clone(), payload);
                }
                Err(e) => {
                    log::warn!("Failed to publish attributes to {topic}: {e}");
                }
            }
        }
    }

    async fn get_current_state(&mut self) -> SimMonitorState {
        match self.iracing.get_current_session_state().await {
            Some(mut session_state) => {
                if let Some(info) = session_state.session_info.take() {
                    self.session_info = Some(info);
                }

                let session_type = self
                    .session_info
                    .as_ref()
                    .and_then(|doc| resolve_path(doc, SESSION_TYPE_PATH));
                let session_type_enum = match session_type.as_ref().and_then(|v| v.as_str()) {
                    Some("Practice") => SessionType::Practice,
                    Some("Qualify") => SessionType::Qualify,
                    Some("Race") => SessionType::Race,
                    Some("Lone Qualify") => SessionType::LoneQualify,
                    Some("Offline Testing") => SessionType::OfflineTesting,
                    unknown => {
                        log::warn!("Unknown session type received: {:?}", unknown);
                        SessionType::Disconnected
                    }
                };

                let now = Utc::now();
                let session_time_remaining =
                    session_state.time_remaining.map(|seconds| seconds as i64);
                let session_end_time = session_time_remaining.map(|seconds| {
                    (now + TimeDelta::seconds(seconds)).to_rfc3339_opts(SecondsFormat::Secs, true)
                });

                SimMonitorState {
                    connected: true,
                    current_session_type: session_type_enum,
                    session_time_remaining,
                    session_end_time,
                    timestamp: now.to_rfc3339(),
                }
            }
            None => SimMonitorState {
                connected: false,
                current_session_type: SessionType::Disconnected,
                session_time_remaining: None,
                session_end_time: None,
                timestamp: Utc::now().to_rfc3339(),
            },
        }
    }
    // // Add a cleanup method
    // pub async fn cleanup(&mut self) {
    //     if let Some(handle) = self.mqtt_eventloop_handle.take() {
    //         handle.abort();
    //     }
    //     if let Some(mqtt) = self.mqtt.take() {
    //         if let Err(e) = mqtt.disconnect().await {
    //             log::warn!("Error disconnecting MQTT client: {}", e);
    //         }
    //     }
    // }
}

impl Drop for SimMonitor {
    fn drop(&mut self) {
        if let Some(handle) = self.mqtt_eventloop_handle.take() {
            handle.abort();
        }
        // For MQTT client, just force close without trying to do a clean disconnect
        self.mqtt = None;
        log::info!("SimMonitor cleanup completed");
    }
}

/// Resolves a path like `SessionInfo.Sessions[{CurrentSessionNum}].SessionType` against
/// a JSON document. `[N]` indexes arrays; `[{Other.Path}]` resolves the index from
/// elsewhere in the same document.
fn resolve_path(root: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    let mut current = root;
    for segment in split_segments(path) {
        let (key, indices) = split_indices(segment)?;
        if !key.is_empty() {
            current = current.get(key)?;
        }
        for index in indices {
            let idx = match index.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                Some(inner) => resolve_path(root, inner)?.as_u64()? as usize,
                None => index.parse::<usize>().ok()?,
            };
            current = current.get(idx)?;
        }
    }
    Some(current.clone())
}

/// Splits a path on `.` separators, ignoring dots inside `[...]`.
fn split_segments(path: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (i, c) in path.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            '.' if depth == 0 => {
                segments.push(&path[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    segments.push(&path[start..]);
    segments
}

/// Splits `Key[a][b]` into `("Key", ["a", "b"])`, `None` on malformed brackets.
fn split_indices(segment: &str) -> Option<(&str, Vec<&str>)> {
    let key_end = segment.find('[').unwrap_or(segment.len());
    let key = &segment[..key_end];
    let mut indices = Vec::new();
    let mut rest = &segment[key_end..];
    while !rest.is_empty() {
        let inner = rest.strip_prefix('[')?;
        let mut depth = 1usize;
        let mut end = None;
        for (i, c) in inner.char_indices() {
            match c {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = end?;
        indices.push(&inner[..end]);
        rest = &inner[end + 1..];
    }
    Some((key, indices))
}

async fn register_device(mqtt: &mut AsyncClient, attribute_groups: &[String]) -> Result<()> {
    // Device-based discovery: a single retained payload at
    // homeassistant/device/<id>/config describing every sensor as a component.

    // Get all session types as strings using serde serialization
    let options: Vec<String> = SessionType::iter()
        .map(|st| {
            serde_json::to_value(&st)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();

    let mut components = serde_json::Map::new();
    components.insert(
        "session_type".to_string(),
        serde_json::json!({
            "platform": "sensor",
            "name": "Session type",
            "state_topic": STATE_TOPIC,
            "value_template": "{{ value_json.current_session_type }}",
            "unique_id": "iracing_session_type",
            "expire_after": 30,
            "icon": "mdi:racing-helmet",
            "device_class": "enum",
            "options": options,
        }),
    );
    components.insert(
        "session_time_remaining".to_string(),
        serde_json::json!({
            "platform": "sensor",
            "name": "Session time remaining",
            "state_topic": STATE_TOPIC,
            // null renders to 'None', which resets the sensor to unknown (HA >= 2025.1)
            "value_template": "{{ value_json.session_time_remaining }}",
            "unique_id": "iracing_session_time_remaining",
            "expire_after": 30,
            "icon": "mdi:timer-outline",
            "device_class": "duration",
            "unit_of_measurement": "s",
        }),
    );
    components.insert(
        "session_end_time".to_string(),
        serde_json::json!({
            "platform": "sensor",
            "name": "Session end time",
            "state_topic": STATE_TOPIC,
            // null renders to 'None', which resets the sensor to unknown (HA >= 2025.1)
            "value_template": "{{ value_json.session_end_time }}",
            "unique_id": "iracing_session_end_time",
            "expire_after": 30,
            "icon": "mdi:flag-checkered",
            "device_class": "timestamp",
        }),
    );

    // One sensor per attribute group: state is the last update time, the
    // group's fields are exposed as attributes via json_attributes_topic.
    for group in attribute_groups {
        let mut chars = group.chars();
        let Some(first) = chars.next() else { continue };
        let display_name = format!("{}{}", first.to_uppercase(), chars.as_str());
        let topic = format!("{TOPIC_PREFIX}/{group}");
        components.insert(
            group.clone(),
            serde_json::json!({
                "platform": "sensor",
                "name": display_name,
                "state_topic": topic,
                "value_template": "{{ value_json.timestamp }}",
                "device_class": "timestamp",
                "json_attributes_topic": topic,
                "unique_id": format!("iracing_{group}"),
                "icon": "mdi:car-info",
            }),
        );
    }

    let config = serde_json::json!({
        "device": {
            "identifiers": ["iracing_ha_monitor"],
            "name": "iRacing Simulator",
            "sw_version": env!("CARGO_PKG_VERSION"),
        },
        "origin": {
            "name": "iracing-ha-monitor",
            "sw_version": env!("CARGO_PKG_VERSION"),
        },
        "components": components,
    });

    mqtt.publish(
        DISCOVERY_TOPIC,
        QoS::AtLeastOnce,
        true,
        serde_json::to_string(&config)?,
    )
    .await
    .context("Failed to publish MQTT discovery configuration")?;

    log::info!("Registered device with Home Assistant.");
    Ok(())
}

// messages to SimMonitor
#[derive(Debug, Clone)]
pub enum Message {
    UpdateConfig(config::AppConfig),
}

#[derive(Debug, Clone)]
pub struct Connection(mpsc::Sender<Message>);

impl Connection {
    pub fn send(&mut self, message: Message) {
        self.0.try_send(message).expect("Send message SimMonitor");
    }
}

// events from SimMonitor
#[derive(Debug, Clone)]
pub enum Event {
    Ready(Connection),
    ConnectedToSim(SimMonitorState),
    DisconnectedFromSim(SimMonitorState),
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Event::Ready(_) => write!(f, "Ready to connect to iRacing"),
            Event::Ready(_connection) => write!(f, "Ready"),
            Event::DisconnectedFromSim(_) => write!(f, "iRacing Disconnected"),
            Event::ConnectedToSim(_) => write!(f, "iRacing Connected"),
        }
    }
}

pub fn connect(config: Option<AppConfig>) -> impl Stream<Item = Event> {
    // Create the monitor
    let attributes_config = config
        .as_ref()
        .map(|c| c.attributes.clone())
        .unwrap_or_default();
    let mqtt_config = config.and_then(|c| if c.mqtt_enabled { Some(c.mqtt) } else { None });
    let mut monitor = SimMonitor::new(mqtt_config, attributes_config);

    iced_stream::channel(100, |mut output| async move {
        // Create channel
        let (sender, mut receiver) = mpsc::channel(100);

        // Start the MQTT event loop
        monitor.start_mqtt_eventloop().await;

        // Sim state update interval
        const UPDATE_INTERVAL: Duration = Duration::from_secs(1);
        let mut interval = tokio::time::interval(UPDATE_INTERVAL);

        // Get the initial state
        // let mut previous_state = monitor.get_current_state().await;
        let mut previous_state = SimMonitorState::default();

        // Send the sender back to the application
        output
            .send(Event::Ready(Connection(sender)))
            .await
            .expect("Unable to send");

        loop {
            tokio::select! {
                // Handle incoming messages
                Some(input) = receiver.next() => {
                    match input {
                        Message::UpdateConfig(config) => {
                            log::debug!("Received config update");
                            if monitor.attributes_config != config.attributes {
                                monitor.attributes_config = config.attributes.clone();
                                // Re-publish all groups with the new projection
                                monitor.last_attributes.clear();
                            }
                            if config.mqtt_enabled {
                                log::info!("Updating mqtt config");
                                monitor.set_mqtt_config(Some(config.mqtt));
                                monitor.start_mqtt_eventloop().await;
                            } else if monitor.mqtt.is_some() {
                                log::info!("Disabling MQTT");
                                monitor.set_mqtt_config(None);
                            }
                        }
                    }
                }
                // Periodic state update
                _ = interval.tick() => {
                    let state = monitor.get_current_state().await;
                    log::debug!("Latest state: {:?}", state);
                    if let Err(e) = monitor.publish_state(&state).await {
                        log::warn!("Failed to publish state to MQTT: {}", e);
                    }

                    // Publish state event
                    let event = if state.connected {
                        Event::ConnectedToSim(state.clone())
                    } else {
                        Event::DisconnectedFromSim(state.clone())
                    };
                    if let Err(e) = output.send(event).await {
                        log::error!("Failed to send state event: {}", e);
                        // break; // Consider breaking the loop if we can't send events
                    }

                    // Check if the state has changed
                    if state.current_session_type != previous_state.current_session_type || state.connected != previous_state.connected {
                        log::info!("State changed, new state: {:?}", state);
                    }
                    previous_state = state;
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::resolve_path;
    use serde_json::json;

    fn doc() -> serde_json::Value {
        json!({
            "CurrentSessionNum": 1,
            "WeekendInfo": { "TrackDisplayName": "Mock Raceway" },
            "SessionInfo": {
                "Sessions": [
                    { "SessionNum": 0, "SessionType": "Practice" },
                    { "SessionNum": 1, "SessionType": "Race" },
                ],
            },
            "DriverInfo": {
                "DriverCarIdx": 1,
                "Drivers": [
                    { "UserName": "Other Driver" },
                    { "UserName": "Mock Driver" },
                ],
            },
        })
    }

    #[test]
    fn resolves_plain_keys() {
        assert_eq!(
            resolve_path(&doc(), "WeekendInfo.TrackDisplayName"),
            Some(json!("Mock Raceway"))
        );
    }

    #[test]
    fn resolves_literal_index() {
        assert_eq!(
            resolve_path(&doc(), "SessionInfo.Sessions[0].SessionType"),
            Some(json!("Practice"))
        );
    }

    #[test]
    fn resolves_dynamic_index() {
        assert_eq!(
            resolve_path(
                &doc(),
                "SessionInfo.Sessions[{CurrentSessionNum}].SessionType"
            ),
            Some(json!("Race"))
        );
        assert_eq!(
            resolve_path(
                &doc(),
                "DriverInfo.Drivers[{DriverInfo.DriverCarIdx}].UserName"
            ),
            Some(json!("Mock Driver"))
        );
    }

    #[test]
    fn resolves_whole_subtree() {
        assert_eq!(
            resolve_path(&doc(), "DriverInfo.Drivers"),
            Some(json!([
                { "UserName": "Other Driver" },
                { "UserName": "Mock Driver" },
            ]))
        );
    }

    #[test]
    fn missing_or_malformed_paths_return_none() {
        assert_eq!(resolve_path(&doc(), "WeekendInfo.Nope"), None);
        assert_eq!(
            resolve_path(&doc(), "SessionInfo.Sessions[9].SessionType"),
            None
        );
        assert_eq!(resolve_path(&doc(), "SessionInfo.Sessions[oops]"), None);
        assert_eq!(resolve_path(&doc(), "SessionInfo.Sessions[0"), None);
    }
}
