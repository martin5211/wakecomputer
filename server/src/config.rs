use serde::Deserialize;

use crate::errors::AppError;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub listen: Option<String>,
    pub api_token: String,
    pub machines: Vec<Machine>,
}

#[derive(Debug, Deserialize)]
pub struct Machine {
    pub name: String,
    pub mac: String,
    pub ip: String,
    #[serde(default = "default_agent_port")]
    pub agent_port: Option<u16>,
    pub agent_token: String,
}

fn default_agent_port() -> Option<u16> {
    Some(9877)
}

impl Config {
    pub fn find_machine(&self, name: &str) -> Option<&Machine> {
        self.machines
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(name))
    }
}

pub fn load_config(path: &str) -> Result<Config, AppError> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| AppError::Config(format!("{path}: {e}")))?;

    let config: Config =
        serde_yaml_ng::from_str(&contents).map_err(|e| AppError::Config(e.to_string()))?;

    for machine in &config.machines {
        validate_mac(&machine.mac).map_err(|e| {
            AppError::Config(format!("machine '{}': {}", machine.name, e))
        })?;
    }

    Ok(config)
}

fn validate_mac(mac: &str) -> Result<(), String> {
    let cleaned: Vec<&str> = mac.split(|c| c == ':' || c == '-').collect();
    if cleaned.len() != 6 {
        return Err(format!("invalid MAC address: {mac}"));
    }
    for octet in &cleaned {
        if octet.len() != 2 || !octet.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("invalid MAC address: {mac}"));
        }
    }
    Ok(())
}
