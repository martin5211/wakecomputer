use std::net::SocketAddr;

use serde::Deserialize;

use crate::errors::AppError;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub listen: Option<String>,
    pub api_token: String,
    #[serde(default)]
    pub wol: WolConfig,
    pub machines: Vec<Machine>,
}

#[derive(Debug, Deserialize)]
pub struct WolConfig {
    #[serde(default = "default_broadcast_address")]
    pub broadcast_address: String,
    #[serde(default)]
    pub multicast_enabled: bool,
}

impl Default for WolConfig {
    fn default() -> Self {
        Self {
            broadcast_address: default_broadcast_address(),
            multicast_enabled: false,
        }
    }
}

fn default_broadcast_address() -> String {
    "255.255.255.255:9".to_string()
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

    validate_wol(&config.wol)?;

    for machine in &config.machines {
        validate_mac(&machine.mac).map_err(|e| {
            AppError::Config(format!("machine '{}': {}", machine.name, e))
        })?;
    }

    Ok(config)
}

fn validate_wol(wol: &WolConfig) -> Result<(), AppError> {
    let addr: SocketAddr = wol
        .broadcast_address
        .parse()
        .map_err(|e| AppError::Config(format!("wol.broadcast_address '{}': {e}", wol.broadcast_address)))?;

    let ip = match addr.ip() {
        std::net::IpAddr::V4(v4) => v4,
        _ => {
            return Err(AppError::Config(format!(
                "wol.broadcast_address '{}': IPv6 not supported",
                wol.broadcast_address
            )));
        }
    };

    if wol.multicast_enabled {
        if !ip.is_multicast() {
            return Err(AppError::Config(format!(
                "wol.broadcast_address '{}': expected multicast address (224.0.0.0/4)",
                wol.broadcast_address
            )));
        }
    } else {
        let octets = ip.octets();
        if octets[3] != 255 {
            return Err(AppError::Config(format!(
                "wol.broadcast_address '{}': expected broadcast address (last octet must be 255)",
                wol.broadcast_address
            )));
        }
    }

    Ok(())
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
