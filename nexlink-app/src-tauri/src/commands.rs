use crate::state::{AppCommand, AppState, PeerInfo, ProxyStatus, SharedState, TrafficStats};
use tauri::State;
use tokio::sync::oneshot;

#[tauri::command]
pub async fn get_identity(state: State<'_, AppState>) -> Result<String, String> {
    let shared = state.shared.read().await;
    Ok(shared.peer_id.clone())
}

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<SharedState, String> {
    let shared = state.shared.read().await;
    Ok(shared.clone())
}

#[tauri::command]
pub async fn list_nodes(state: State<'_, AppState>) -> Result<Vec<PeerInfo>, String> {
    state
        .cmd_tx
        .send(AppCommand::RefreshNodes)
        .await
        .map_err(|e| e.to_string())?;
    let shared = state.shared.read().await;
    Ok(shared.discovered_peers.clone())
}

#[tauri::command]
pub async fn connect_node(state: State<'_, AppState>, peer_id: String) -> Result<(), String> {
    state
        .cmd_tx
        .send(AppCommand::ConnectNode { peer_id })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disconnect_node(state: State<'_, AppState>) -> Result<(), String> {
    state
        .cmd_tx
        .send(AppCommand::DisconnectNode)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_proxy(
    state: State<'_, AppState>,
    unified_port: u16,
) -> Result<(), String> {
    let (tx, rx) = oneshot::channel();
    state
        .cmd_tx
        .send(AppCommand::StartProxy {
            unified_port,
            done: tx,
        })
        .await
        .map_err(|e| e.to_string())?;
    rx.await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn stop_proxy(state: State<'_, AppState>) -> Result<(), String> {
    let (tx, rx) = oneshot::channel();
    state
        .cmd_tx
        .send(AppCommand::StopProxy { done: tx })
        .await
        .map_err(|e| e.to_string())?;
    rx.await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_traffic(state: State<'_, AppState>) -> Result<TrafficStats, String> {
    let shared = state.shared.read().await;
    Ok(shared.traffic.clone())
}

#[tauri::command]
pub async fn get_proxy_status(
    state: State<'_, AppState>,
) -> Result<Option<ProxyStatus>, String> {
    let shared = state.shared.read().await;
    Ok(shared.proxy_status.clone())
}

#[tauri::command]
pub async fn update_config(
    state: State<'_, AppState>,
    relay_addr: Option<String>,
    namespace: Option<String>,
) -> Result<(), String> {
    let (tx, rx) = oneshot::channel();
    state
        .cmd_tx
        .send(AppCommand::UpdateConfig {
            relay_addr,
            namespace,
            done: tx,
        })
        .await
        .map_err(|e| e.to_string())?;
    rx.await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn join_network(
    state: State<'_, AppState>,
    name: String,
    password: String,
) -> Result<(), String> {
    state
        .cmd_tx
        .send(AppCommand::JoinNetwork { name, password })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn leave_network(state: State<'_, AppState>) -> Result<(), String> {
    state
        .cmd_tx
        .send(AppCommand::LeaveNetwork)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_system_proxy(state: State<'_, AppState>) -> Result<(), String> {
    let (tx, rx) = oneshot::channel();
    state
        .cmd_tx
        .send(AppCommand::SetSystemProxy { done: tx })
        .await
        .map_err(|e| e.to_string())?;
    rx.await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn clear_system_proxy(state: State<'_, AppState>) -> Result<(), String> {
    let (tx, rx) = oneshot::channel();
    state
        .cmd_tx
        .send(AppCommand::ClearSystemProxy { done: tx })
        .await
        .map_err(|e| e.to_string())?;
    rx.await.map_err(|e| e.to_string())?
}
