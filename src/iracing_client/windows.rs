use crate::iracing_client::{SessionState, SimClient}; // Make sure to import the trait
use simetry::iracing;
use std::time::Duration;
use tokio::time::timeout;
use yaml_rust::Yaml;

/// iRacing reports one week of remaining time for untimed sessions.
const UNLIMITED_TIME: f64 = 604_800.0;

pub struct IracingClient {
    client: Option<iracing::Client>,
    last_session_info: Option<Yaml>,
    last_session_num: Option<i32>,
}

impl IracingClient {
    fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    async fn connect(&mut self) -> bool {
        if !self.is_connected() {
            log::debug!("Waiting for iRacing connection...");
            let connect_result =
                timeout(Duration::from_secs(5), iracing::Client::try_connect()).await;

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
}

/// Schema-agnostic conversion of the session info YAML into JSON.
fn yaml_to_json(yaml: &Yaml) -> serde_json::Value {
    use serde_json::Value;
    match yaml {
        Yaml::Real(s) => s
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(s.clone())),
        Yaml::Integer(i) => Value::from(*i),
        Yaml::String(s) => Value::String(s.clone()),
        Yaml::Boolean(b) => Value::Bool(*b),
        Yaml::Array(items) => Value::Array(items.iter().map(yaml_to_json).collect()),
        Yaml::Hash(hash) => Value::Object(
            hash.iter()
                .filter_map(|(key, value)| {
                    key.as_str()
                        .map(|key| (key.to_string(), yaml_to_json(value)))
                })
                .collect(),
        ),
        _ => Value::Null,
    }
}

#[async_trait::async_trait]
impl SimClient for IracingClient {
    fn new() -> Self {
        Self {
            client: None,
            last_session_info: None,
            last_session_num: None,
        }
    }

    async fn get_current_session_state(&mut self) -> Option<SessionState> {
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
                // Force a session info re-publish on reconnect
                self.last_session_info = None;
                self.last_session_num = None;
                return None;
            }
        };
        let session_info = sim_state.session_info();
        let session_num = sim_state.read_name::<i32>("SessionNum")?;

        let time_remaining = sim_state
            .read_name::<f64>("SessionTimeRemain")
            .filter(|remaining| remaining.is_finite() && (0.0..UNLIMITED_TIME).contains(remaining));

        let session_info_json = if self.last_session_info.as_ref() != Some(session_info)
            || self.last_session_num != Some(session_num)
        {
            self.last_session_info = Some(session_info.clone());
            self.last_session_num = Some(session_num);
            let mut json = yaml_to_json(session_info);
            if let serde_json::Value::Object(map) = &mut json {
                // Expose the current session number for `[{CurrentSessionNum}]` paths
                map.insert("CurrentSessionNum".to_string(), session_num.into());
            }
            Some(json)
        } else {
            None
        };

        Some(SessionState {
            time_remaining,
            session_info: session_info_json,
        })
    }
}
