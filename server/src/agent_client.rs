use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Command;

use crate::config::Machine;
use crate::errors::AppError;

pub async fn shutdown_machine(machine: &Machine) -> Result<(), AppError> {
    let addr = agent_addr(machine);
    let token = &machine.agent_token;

    let request = format!(
        "POST /shutdown HTTP/1.1\r\n\
         Host: {addr}\r\n\
         Authorization: Bearer {token}\r\n\
         Connection: close\r\n\
         \r\n"
    );

    let mut stream = match TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(_) => {
            if ping(&machine.ip).await {
                return Err(AppError::Agent(format!(
                    "agent on {} is not running but machine responds to ping",
                    machine.name
                )));
            }
            tracing::info!(machine = machine.name, "machine is already off (no agent, no ping)");
            return Ok(());
        }
    };

    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| AppError::Agent(format!("write {addr}: {e}")))?;

    // Read whatever response we can — the machine may die mid-response
    let mut buf = vec![0u8; 4096];
    match stream.read(&mut buf).await {
        Ok(n) => {
            let response = String::from_utf8_lossy(&buf[..n]);
            if response.contains("200") {
                tracing::info!(machine = machine.name, "shutdown command accepted");
            } else if response.contains("401") {
                return Err(AppError::Agent(format!(
                    "agent on {} rejected token (401)",
                    machine.name
                )));
            } else {
                tracing::warn!(
                    machine = machine.name,
                    response = %response.lines().next().unwrap_or(""),
                    "unexpected agent response, shutdown may still proceed"
                );
            }
        }
        Err(_) => {
            // Connection reset — machine is shutting down, treat as success
            tracing::info!(
                machine = machine.name,
                "connection reset after shutdown request (expected)"
            );
        }
    }

    Ok(())
}

pub async fn check_agent_health(machine: &Machine) -> Result<bool, AppError> {
    let addr = agent_addr(machine);

    let request = format!(
        "GET /health HTTP/1.1\r\n\
         Host: {addr}\r\n\
         Connection: close\r\n\
         \r\n"
    );

    let mut stream = match TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(_) => return Ok(ping(&machine.ip).await),
    };

    if stream.write_all(request.as_bytes()).await.is_err() {
        return Ok(ping(&machine.ip).await);
    }

    let mut buf = vec![0u8; 4096];
    match stream.read(&mut buf).await {
        Ok(n) => {
            let response = String::from_utf8_lossy(&buf[..n]);
            Ok(response.contains("200"))
        }
        Err(_) => Ok(ping(&machine.ip).await),
    }
}

fn agent_addr(machine: &Machine) -> String {
    let port = machine.agent_port.unwrap_or(9877);
    format!("{}:{}", machine.ip, port)
}

async fn ping(ip: &str) -> bool {
    Command::new("ping")
        .args(["-c", "1", "-W", "1", ip])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .is_ok_and(|s| s.success())
}
