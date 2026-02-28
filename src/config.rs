use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Which device to grab
    #[serde(default)]
    pub device: DeviceConfig,

    /// Named profiles
    #[serde(default)]
    pub profiles: Vec<Profile>,

    /// Which profile is active (by name)
    #[serde(default)]
    pub active_profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceConfig {
    /// Match device by name substring (e.g. "G502")
    pub name: Option<String>,
    /// Match device by path (e.g. "/dev/input/event5")
    pub path: Option<String>,
    /// Match by vendor ID
    pub vendor_id: Option<u16>,
    /// Match by product ID
    pub product_id: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub bindings: Vec<Binding>,
    #[serde(default)]
    pub macros: Vec<MacroDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    /// Input event code name, e.g. "BTN_LEFT", "BTN_EXTRA", "BTN_SIDE"
    pub input: String,
    /// What to do when this button is pressed
    pub output: BindingOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BindingOutput {
    /// Remap to a different key/button
    Key { key: String },
    /// Trigger a named macro
    Macro { macro_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroDef {
    pub name: String,
    #[serde(rename = "type")]
    pub macro_type: MacroType,
    /// Actions to perform
    pub actions: Vec<MacroAction>,
    /// For repeat_on_hold: interval between repeats in milliseconds
    #[serde(default = "default_interval")]
    pub interval_ms: u64,
    /// Optional initial delay before first repeat
    #[serde(default)]
    pub initial_delay_ms: u64,
}

fn default_interval() -> u64 {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MacroType {
    /// Fire actions repeatedly while the trigger button is held
    RepeatOnHold,
    /// Fire a sequence of actions once on button press
    Sequence,
    /// Toggle: first press starts repeating, second press stops
    Toggle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroAction {
    /// Click a button (press + release)
    Click(String),
    /// Press a key/button (down only)
    Press(String),
    /// Release a key/button (up only)
    Release(String),
    /// Wait for a duration in milliseconds
    Delay(u64),
}

impl Config {
    /// Load config from the default path (~/.config/mouse-mapper/config.toml)
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config from {}", path.display()))?;
            let config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config from {}", path.display()))?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Save config to the default path
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config dir {}", parent.display()))?;
        }
        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write config to {}", path.display()))?;
        Ok(())
    }

    fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("Could not determine config directory")?;
        Ok(config_dir.join("mouse-mapper").join("config.toml"))
    }

    /// Get the active profile
    pub fn active_profile(&self) -> Option<&Profile> {
        if let Some(ref name) = self.active_profile {
            self.profiles.iter().find(|p| &p.name == name)
        } else {
            self.profiles.first()
        }
    }

    /// Get mutable active profile
    pub fn active_profile_mut(&mut self) -> Option<&mut Profile> {
        if let Some(ref name) = self.active_profile {
            let name = name.clone();
            self.profiles.iter_mut().find(|p| p.name == name)
        } else {
            self.profiles.first_mut()
        }
    }

    /// Build a lookup map: input code name -> BindingOutput for the active profile
    pub fn build_binding_map(&self) -> HashMap<String, BindingOutput> {
        let mut map = HashMap::new();
        if let Some(profile) = self.active_profile() {
            for binding in &profile.bindings {
                map.insert(binding.input.clone(), binding.output.clone());
            }
        }
        map
    }

    /// Build a lookup map: macro name -> MacroDef for the active profile
    pub fn build_macro_map(&self) -> HashMap<String, MacroDef> {
        let mut map = HashMap::new();
        if let Some(profile) = self.active_profile() {
            for m in &profile.macros {
                map.insert(m.name.clone(), m.clone());
            }
        }
        map
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            device: DeviceConfig::default(),
            profiles: vec![Profile {
                name: "Default".to_string(),
                bindings: vec![],
                macros: vec![],
            }],
            active_profile: Some("Default".to_string()),
        }
    }
}
