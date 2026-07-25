use std::{sync::mpsc::Receiver, thread, time::Duration};

use tracing::info;

use crate::{Result, daemon::InputEvent};

pub fn translate_controller_input_to_mouse_movements(rx: Receiver<InputEvent>) -> Result<()> {
    loop {
        while let Ok(event) = rx.try_recv() {
            match event {
                InputEvent::Axis {
                    axis,
                    value,
                    centred,
                } => {
                    if !centred {
                        info!("Received Cursor manipulation events from worker thread!");
                        info!(?axis, value);
                    }
                }
                InputEvent::Button { key, action } => {
                    info!("Received button events from worker thread!");
                    info!(?key, ?action);
                }
            }
        }

        thread::sleep(Duration::from_millis(8));
    }
}
