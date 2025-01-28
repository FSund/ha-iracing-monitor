pub use async_trait::async_trait;

#[async_trait]
pub trait SimClient {
    fn new() -> Self;
    async fn connect(&mut self) -> bool;
    // fn is_connected(&self) -> bool;
    async fn get_current_session_type(&mut self) -> Option<String>;
}

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::IracingClient as Client;

#[cfg(not(target_os = "windows"))]
mod mock;
#[cfg(not(target_os = "windows"))]
pub use mock::MockClient as Client;
