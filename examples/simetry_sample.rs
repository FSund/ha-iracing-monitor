use simetry::iracing::Client;
use std::time::Duration;
use yaml_rust::{Yaml, YamlEmitter};
use std::fs::File;
use std::io::Write;

fn get_current_session_type(session_info: &Yaml, session_num: i32) -> Option<String> {
    // Navigate through the YAML structure to get Sessions array
    if let Some(sessions) = session_info["SessionInfo"]["Sessions"].as_vec() {
        // Find the session matching our SessionNum
        for session in sessions {
            // println!("sessionNum: {:?}", session["SessionNum"]);
            if let Some(num) = session["SessionNum"].as_i64() {
                // println!("SessionNum: {}", num);
                if num as i32 == session_num {
                    // Found matching session, get its type
                    if let Some(session_type) = session["SessionType"].as_str() {
                        return Some(session_type.to_string());
                    }
                }
            }
        }
    }
    None
}

#[tokio::main]
async fn main() {
    loop {
        println!("Starting connection to iRacing...");
        let mut client = Client::connect(Duration::from_secs(1)).await;
        println!("Connected!");
        while let Some(sim_state) = client.next_sim_state().await {
            // Get session info which contains YAML data
            let session_info = sim_state.session_info();

            // write_yaml_to_file(session_info, "session_info.yaml").expect("Failed to write to file");
            
            // Get the current SessionNum from telemetry
            if let Some(session_num) = sim_state.read_name("SessionNum") {
                if let Some(session_type) = get_current_session_type(session_info, session_num) {
                    println!("Session type: {}", session_type);
                }
            }

            let rpm = f32::round(sim_state.read_name("RPM").unwrap_or(0.0));
            let speed = f32::round(sim_state.read_name("Speed").unwrap_or(0.0) * 3.6);
            println!("{} km/h @ {} RPM", speed, rpm);
        }
        println!("Connection finished!");
    }
}
