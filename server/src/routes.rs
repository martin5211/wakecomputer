use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::agent_client;
use crate::config::Config;
use crate::errors::AppError;
use crate::wol;

pub fn app(config: Arc<Config>) -> Router {
    Router::new()
        .route("/machines", get(list_machines))
        .route("/wake", post(wake))
        .route("/shutdown", post(shutdown))
        .route("/status", post(status))
        .with_state(config)
}

fn check_token(headers: &HeaderMap, config: &Config) -> Result<(), Response> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "missing or invalid Authorization header"})),
            )
                .into_response()
        })?;

    if token != config.api_token {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid token"})),
        )
            .into_response());
    }

    Ok(())
}

async fn list_machines(
    State(config): State<Arc<Config>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, Response> {
    check_token(&headers, &config)?;

    let names: Vec<&str> = config.machines.iter().map(|m| m.name.as_str()).collect();
    Ok(Json(json!({ "machines": names })))
}

#[derive(Deserialize)]
struct MachineRequest {
    machine: String,
}

async fn wake(
    State(config): State<Arc<Config>>,
    headers: HeaderMap,
    Json(req): Json<MachineRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    check_token(&headers, &config)?;

    let m = config
        .find_machine(&req.machine)
        .ok_or_else(|| AppError::MachineNotFound(req.machine.clone()).into_response())?;

    wol::send_magic_packet(&m.mac)
        .await
        .map_err(|e| e.into_response())?;

    Ok(Json(json!({ "status": "ok", "action": "wake", "machine": req.machine })))
}

async fn shutdown(
    State(config): State<Arc<Config>>,
    headers: HeaderMap,
    Json(req): Json<MachineRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    check_token(&headers, &config)?;

    let m = config
        .find_machine(&req.machine)
        .ok_or_else(|| AppError::MachineNotFound(req.machine.clone()).into_response())?;

    agent_client::shutdown_machine(m)
        .await
        .map_err(|e| e.into_response())?;

    Ok(Json(json!({ "status": "ok", "action": "shutdown", "machine": req.machine })))
}

async fn status(
    State(config): State<Arc<Config>>,
    headers: HeaderMap,
    Json(req): Json<MachineRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    check_token(&headers, &config)?;

    let m = config
        .find_machine(&req.machine)
        .ok_or_else(|| AppError::MachineNotFound(req.machine.clone()).into_response())?;

    let is_on = agent_client::check_agent_health(m)
        .await
        .map_err(|e| e.into_response())?;

    let power = if is_on { "on" } else { "off" };
    Ok(Json(json!({ "status": "ok", "machine": req.machine, "power": power })))
}
