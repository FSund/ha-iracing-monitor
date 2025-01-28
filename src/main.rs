mod iracing_client;
mod frontend;
mod monitor;

// use iracing_client::SimClient;
// use anyhow::{Context, Result};
// use chrono::Utc;
// use rumqttc::{AsyncClient, MqttOptions, QoS};
// use serde::Serialize;
// use std::time::Duration;
// use tokio::time;
use env_logger::{Builder, Target};
use log::LevelFilter;
// use iced;
use frontend::IracingMonitorGui;

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
    // env_logger::init();
    let mut builder = Builder::from_default_env();
    
    // Set external crates to INFO level
    // builder.filter_module("rumqttc", LevelFilter::Info);
    
    // Keep your application at DEBUG level
    builder.filter_module("iracing_ha_monitor", LevelFilter::Debug);
    
    // Apply the configuration
    builder.target(Target::Stdout)
           .init();

    // using a daemon is overkill for a plain iced application, but might come in
    // handy when trying to implement a tray icon
    iced::daemon(IracingMonitorGui::title, IracingMonitorGui::update, IracingMonitorGui::view)
        .subscription(IracingMonitorGui::subscription)
        .run_with(IracingMonitorGui::new)
}