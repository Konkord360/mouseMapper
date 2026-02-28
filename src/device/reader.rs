use anyhow::{Context, Result};
use evdev::Device;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Wrapper around an evdev device with exclusive grab support.
/// Releasing the grab on Drop ensures the mouse always returns to normal.
pub struct DeviceReader {
    device: Device,
    path: PathBuf,
    grabbed: bool,
}

impl DeviceReader {
    /// Open a device for reading
    pub fn open(path: &Path) -> Result<Self> {
        let device = Device::open(path)
            .with_context(|| format!("Failed to open device {}", path.display()))?;

        log::info!(
            "Opened device: {} ({})",
            device.name().unwrap_or("Unknown"),
            path.display()
        );

        Ok(Self {
            device,
            path: path.to_path_buf(),
            grabbed: false,
        })
    }

    /// Grab the device exclusively. While grabbed, events are only delivered to us,
    /// not to the rest of the system.
    pub fn grab(&mut self) -> Result<()> {
        self.device
            .grab()
            .with_context(|| format!("Failed to grab device {}", self.path.display()))?;
        self.grabbed = true;
        log::info!("Grabbed device: {}", self.path.display());
        Ok(())
    }

    /// Release the exclusive grab
    pub fn ungrab(&mut self) -> Result<()> {
        if self.grabbed {
            self.device
                .ungrab()
                .with_context(|| format!("Failed to ungrab device {}", self.path.display()))?;
            self.grabbed = false;
            log::info!("Released grab on device: {}", self.path.display());
        }
        Ok(())
    }

    pub fn is_grabbed(&self) -> bool {
        self.grabbed
    }

    pub fn name(&self) -> &str {
        self.device.name().unwrap_or("Unknown")
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get a reference to the underlying evdev device
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Read events in a blocking loop and send them through the channel.
    /// This should be called from a blocking tokio task.
    pub fn read_loop(mut self, tx: mpsc::UnboundedSender<evdev::InputEvent>) -> Result<()> {
        loop {
            match self.device.fetch_events() {
                Ok(events) => {
                    for event in events {
                        if tx.send(event).is_err() {
                            // Receiver dropped, shut down
                            log::info!("Event channel closed, stopping reader");
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        continue;
                    }
                    // Check if we should stop
                    log::error!("Error reading events: {}", e);
                    return Err(e.into());
                }
            }
        }
    }
}

impl Drop for DeviceReader {
    fn drop(&mut self) {
        if self.grabbed {
            log::info!("Drop: releasing grab on {}", self.path.display());
            if let Err(e) = self.device.ungrab() {
                log::error!("Failed to ungrab device on drop: {}", e);
            }
        }
    }
}
