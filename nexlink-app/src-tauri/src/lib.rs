mod commands;
mod state;
mod swarm_task;

use state::{AppCommand, AppState, SharedState};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("nexlink_app_lib=debug,nexlink_lib=debug,info")),
        )
        .init();

    let (cmd_tx, cmd_rx) = mpsc::channel::<AppCommand>(32);
    let shared = Arc::new(RwLock::new(SharedState {
        peer_id: "initializing...".to_string(),
        nat_status: "Unknown".to_string(),
        relay_addr: String::new(),
        namespace: "nexlink-public".to_string(),
        data_dir: nexlink_lib::config::default_data_dir()
            .to_string_lossy()
            .to_string(),
        network_mode: "public".to_string(),
        network_name: None,
        ..Default::default()
    }));

    let app_state = AppState {
        cmd_tx,
        shared: shared.clone(),
    };

    tauri::Builder::default()
        .manage(app_state)
        .setup(move |app| {
            let handle = app.handle().clone();
            let shared_clone = shared.clone();
            let data_dir = nexlink_lib::config::default_data_dir()
                .to_string_lossy()
                .to_string();

            tauri::async_runtime::spawn(async move {
                if let Err(e) =
                    swarm_task::run_swarm_task(handle, cmd_rx, shared_clone, data_dir).await
                {
                    tracing::error!("Swarm task error: {e}");
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_identity,
            commands::get_status,
            commands::list_nodes,
            commands::connect_node,
            commands::disconnect_node,
            commands::start_proxy,
            commands::stop_proxy,
            commands::get_traffic,
            commands::get_proxy_status,
            commands::update_config,
            commands::join_network,
            commands::leave_network,
            commands::set_system_proxy,
            commands::clear_system_proxy,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
