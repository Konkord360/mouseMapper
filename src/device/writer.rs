use anyhow::{Context, Result};
use evdev::{
    uinput::VirtualDevice, AttributeSet, InputEvent, KeyCode, RelativeAxisCode, UinputAbsSetup,
};

/// Virtual device that emits events via uinput.
/// Events injected through this device are kernel-level input events,
/// indistinguishable from real hardware to any userspace application.
pub struct DeviceWriter {
    virtual_device: VirtualDevice,
}

impl DeviceWriter {
    /// Create a virtual device that mirrors the capabilities of the given source device.
    pub fn from_source(source: &evdev::Device) -> Result<Self> {
        let mut builder = VirtualDevice::builder()
            .context("Failed to create VirtualDeviceBuilder")?
            .name("MouseMapper Virtual Device");

        // Mirror key/button capabilities
        if let Some(keys) = source.supported_keys() {
            let mut attr = AttributeSet::<KeyCode>::new();
            for key in keys.iter() {
                attr.insert(key);
            }
            // Also add all common keyboard keys so we can remap mouse buttons to keys
            for code in 1..=248u16 {
                attr.insert(KeyCode::new(code));
            }
            builder = builder.with_keys(&attr)?;
        }

        // Mirror relative axis capabilities (mouse movement, scroll)
        if let Some(rel_axes) = source.supported_relative_axes() {
            let mut attr = AttributeSet::<RelativeAxisCode>::new();
            for axis in rel_axes.iter() {
                attr.insert(axis);
            }
            builder = builder.with_relative_axes(&attr)?;
        }

        // Mirror absolute axis capabilities if any
        if let Some(abs_axes) = source.supported_absolute_axes() {
            for axis in abs_axes.iter() {
                if let Some(info) = source.get_abs_state()?.get(axis.0 as usize) {
                    let setup = UinputAbsSetup::new(
                        axis,
                        evdev::AbsInfo::new(
                            info.value,
                            info.minimum,
                            info.maximum,
                            info.fuzz,
                            info.flat,
                            info.resolution,
                        ),
                    );
                    builder = builder.with_absolute_axis(&setup)?;
                }
            }
        }

        let virtual_device = builder.build().context("Failed to build virtual device")?;

        log::info!("Created virtual device: MouseMapper Virtual Device");

        Ok(Self { virtual_device })
    }

    /// Create a virtual device with standard mouse + keyboard capabilities.
    /// Used when we don't have a source device to mirror.
    pub fn new_standard() -> Result<Self> {
        let mut keys = AttributeSet::<KeyCode>::new();
        // All mouse buttons
        keys.insert(KeyCode::BTN_LEFT);
        keys.insert(KeyCode::BTN_RIGHT);
        keys.insert(KeyCode::BTN_MIDDLE);
        keys.insert(KeyCode::BTN_SIDE);
        keys.insert(KeyCode::BTN_EXTRA);
        keys.insert(KeyCode::BTN_FORWARD);
        keys.insert(KeyCode::BTN_BACK);
        keys.insert(KeyCode::BTN_TASK);
        // Common keyboard keys
        for code in 1..=248u16 {
            keys.insert(KeyCode::new(code));
        }

        let mut rel = AttributeSet::<RelativeAxisCode>::new();
        rel.insert(RelativeAxisCode::REL_X);
        rel.insert(RelativeAxisCode::REL_Y);
        rel.insert(RelativeAxisCode::REL_WHEEL);
        rel.insert(RelativeAxisCode::REL_HWHEEL);
        rel.insert(RelativeAxisCode::REL_WHEEL_HI_RES);
        rel.insert(RelativeAxisCode::REL_HWHEEL_HI_RES);

        let virtual_device = VirtualDevice::builder()
            .context("Failed to create VirtualDeviceBuilder")?
            .name("MouseMapper Virtual Device")
            .with_keys(&keys)?
            .with_relative_axes(&rel)?
            .build()
            .context("Failed to build virtual device")?;

        log::info!("Created standard virtual device");

        Ok(Self { virtual_device })
    }

    /// Emit a slice of events through the virtual device
    pub fn emit(&mut self, events: &[InputEvent]) -> Result<()> {
        self.virtual_device
            .emit(events)
            .context("Failed to emit events through virtual device")?;
        Ok(())
    }

    /// Emit a single event followed by a SYN_REPORT
    pub fn emit_event(&mut self, event: InputEvent) -> Result<()> {
        let syn = InputEvent::new(
            evdev::EventType::SYNCHRONIZATION.0,
            0, // SYN_REPORT
            0,
        );
        self.virtual_device
            .emit(&[event, syn])
            .context("Failed to emit event")?;
        Ok(())
    }

    /// Emit a key/button press (value=1) + release (value=0) with SYN_REPORT after each
    pub fn click(&mut self, key: KeyCode) -> Result<()> {
        let press = InputEvent::new(evdev::EventType::KEY.0, key.code(), 1);
        let release = InputEvent::new(evdev::EventType::KEY.0, key.code(), 0);
        let syn = InputEvent::new(evdev::EventType::SYNCHRONIZATION.0, 0, 0);

        self.virtual_device.emit(&[press, syn])?;
        self.virtual_device.emit(&[release, syn])?;
        Ok(())
    }

    /// Emit a key/button down event
    pub fn press(&mut self, key: KeyCode) -> Result<()> {
        let event = InputEvent::new(evdev::EventType::KEY.0, key.code(), 1);
        self.emit_event(event)
    }

    /// Emit a key/button up event
    pub fn release(&mut self, key: KeyCode) -> Result<()> {
        let event = InputEvent::new(evdev::EventType::KEY.0, key.code(), 0);
        self.emit_event(event)
    }
}
