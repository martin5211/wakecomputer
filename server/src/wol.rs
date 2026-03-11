use tokio::net::UdpSocket;

use crate::errors::AppError;

pub async fn send_magic_packet(mac: &str) -> Result<(), AppError> {
    let mac_bytes = parse_mac(mac)?;

    let mut packet = [0u8; 102];
    packet[..6].fill(0xFF);
    for i in 0..16 {
        let offset = 6 + i * 6;
        packet[offset..offset + 6].copy_from_slice(&mac_bytes);
    }

    let socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| AppError::Wol(format!("bind: {e}")))?;

    socket
        .set_broadcast(true)
        .map_err(|e| AppError::Wol(format!("set_broadcast: {e}")))?;

    socket
        .send_to(&packet, "255.255.255.255:9")
        .await
        .map_err(|e| AppError::Wol(format!("send: {e}")))?;

    tracing::info!(mac = mac, "magic packet sent");
    Ok(())
}

fn parse_mac(mac: &str) -> Result<[u8; 6], AppError> {
    let parts: Vec<&str> = mac.split(|c| c == ':' || c == '-').collect();
    if parts.len() != 6 {
        return Err(AppError::Wol(format!("invalid MAC: {mac}")));
    }

    let mut bytes = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        bytes[i] =
            u8::from_str_radix(part, 16).map_err(|_| AppError::Wol(format!("invalid MAC: {mac}")))?;
    }
    Ok(bytes)
}
