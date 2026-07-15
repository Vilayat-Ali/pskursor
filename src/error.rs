use thiserror::Error;

#[derive(Debug, Error)]
pub enum PskursorError {
    #[error("No controller/gamepad device connected/recognised")]
    NoControllerDeviceConnected,
}
