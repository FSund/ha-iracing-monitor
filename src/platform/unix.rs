use std::io;

pub fn set_run_at_startup(_enable: bool, _exe_path: &str) -> io::Result<()> {
    Ok(())
}

pub fn get_run_on_startup_state() -> io::Result<bool> {
    Ok(false)
}

pub fn toggle_run_on_boot() {
    // No-op for Unix systems
}

pub fn shutdown_signals() -> impl Stream<Item = Event> {
    // Create a dummy stream that never produces any events
    iced_stream::channel(100, |_output| async move {
        // Sleep forever
        std::future::pending::<()>().await;
    })
}
