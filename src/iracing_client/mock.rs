use crate::config::TelemetrySensor;
use crate::iracing_client::{SessionState, SimClient};
use std::collections::BTreeMap;

pub struct MockClient {
    connected: bool,
    session_info_sent: bool,
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
        Self {
            connected: false,
            session_info_sent: false,
        }
    }

    async fn get_current_session_state(
        &mut self,
        telemetry: &BTreeMap<String, TelemetrySensor>,
    ) -> Option<SessionState> {
        if !self.connect().await {
            return None;
        }

        if self.connected {
            let session_info = if self.session_info_sent {
                None
            } else {
                self.session_info_sent = true;
                Some(mock_session_info())
            };
            let telemetry = telemetry
                .iter()
                .map(|(id, sensor)| {
                    let value = match sensor.variable.as_str() {
                        "SessionTimeRemain" => serde_json::json!(1800.0),
                        _ => serde_json::Value::Null,
                    };
                    (id.clone(), value)
                })
                .collect();
            Some(SessionState {
                telemetry,
                session_info,
            })
        } else {
            None
        }
    }
}

/// Canned session info document mirroring the iRacing YAML structure.
fn mock_session_info() -> serde_json::Value {
    serde_json::json!({
        "CurrentSessionNum": 0,
        "WeekendInfo": {
            "TrackDisplayName": "Mock Raceway",
            "TrackConfigName": "Grand Prix",
            "TrackAirTemp": "20.00 C",
            "TrackSurfaceTemp": "25.00 C",
            "EventType": "Practice",
        },
        "SessionInfo": {
            "Sessions": [
                {
                    "SessionNum": 0,
                    "SessionType": "Practice",
                    "SessionLaps": "unlimited",
                },
            ],
        },
        "DriverInfo": {
            "DriverCarIdx": 0,
            "Drivers": [
                {
                    "CarIdx": 0,
                    "UserName": "Mock Driver",
                    "CarScreenName": "Mock GT3",
                    "CarClassShortName": "GT3",
                },
            ],
        },
    })
}
