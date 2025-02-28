use crate::helpers;

use anyhow::{Context, Result};
use std::fs;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{filter::Targets, fmt, prelude::*, Registry};

pub fn setup_logging() -> Result<()> {
    let logs_dir = if cfg!(debug_assertions) {
        // Use current directory for config in debug mode
        std::env::current_dir().unwrap().join("logs")
    } else {
        // Use data directory for config in release mode
        helpers::get_project_dir().data_dir().join("logs")
    };

    // Create logs directory if it doesn't exist
    fs::create_dir_all(&logs_dir).context("Failed to create logs directory")?;

    let log_level = if cfg!(debug_assertions) {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    let stdout_filter = Targets::new()
        .with_default(tracing::Level::ERROR)
        .with_target("iracing_ha_monitor", log_level);

    let file_appender_filter = Targets::new()
        .with_default(tracing::Level::WARN)
        .with_target("iracing_ha_monitor", log_level);

    // Create rolling file appender
    let file_appender =
        RollingFileAppender::new(Rotation::DAILY, logs_dir, "iracing_ha_monitor.log");

    // Configure stdout layer
    let stdout_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_file(true)
        .with_filter(stdout_filter);

    // Configure file layer
    let file_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_file(true)
        .with_ansi(false)
        .with_writer(file_appender)
        .with_filter(file_appender_filter);

    // Combine both layers
    Registry::default()
        .with(stdout_layer)
        .with(file_layer)
        .init();

    Ok(())
}
