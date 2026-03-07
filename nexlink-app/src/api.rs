use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::types::{AppStatus, PeerInfo, ProxyStatus, TrafficStats};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["window", "__TAURI__", "core"])]
    fn invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
}

fn to_promise(val: JsValue) -> Result<js_sys::Promise, String> {
    val.dyn_into::<js_sys::Promise>()
        .map_err(|_| "invoke did not return a Promise".to_string())
}

async fn call<T: serde::de::DeserializeOwned>(cmd: &str, args: JsValue) -> Result<T, String> {
    let raw = invoke(cmd, args)
        .map_err(|e| e.as_string().unwrap_or_else(|| "Unknown error".to_string()))?;

    let result = JsFuture::from(to_promise(raw)?)
        .await
        .map_err(|e| e.as_string().unwrap_or_else(|| "Promise rejected".to_string()))?;

    serde_wasm_bindgen::from_value(result).map_err(|e| e.to_string())
}

async fn call_void(cmd: &str, args: JsValue) -> Result<(), String> {
    let raw = invoke(cmd, args)
        .map_err(|e| e.as_string().unwrap_or_else(|| "Unknown error".to_string()))?;

    JsFuture::from(to_promise(raw)?)
        .await
        .map_err(|e| e.as_string().unwrap_or_else(|| "Promise rejected".to_string()))
        .map(|_| ())
}

fn no_args() -> JsValue {
    serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap()
}

pub async fn get_identity() -> Result<String, String> {
    call::<String>("get_identity", no_args()).await
}

pub async fn get_status() -> Result<AppStatus, String> {
    call::<AppStatus>("get_status", no_args()).await
}

pub async fn list_nodes() -> Result<Vec<PeerInfo>, String> {
    call::<Vec<PeerInfo>>("list_nodes", no_args()).await
}

pub async fn connect_node(peer_id: &str) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        peer_id: &'a str,
    }
    let args = serde_wasm_bindgen::to_value(&Args { peer_id }).map_err(|e| e.to_string())?;
    call_void("connect_node", args).await
}

pub async fn disconnect_node() -> Result<(), String> {
    call_void("disconnect_node", no_args()).await
}

pub async fn start_proxy(unified_port: u16) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        unified_port: u16,
    }
    let args = serde_wasm_bindgen::to_value(&Args {
        unified_port,
    })
    .map_err(|e| e.to_string())?;
    call_void("start_proxy", args).await
}

pub async fn stop_proxy() -> Result<(), String> {
    call_void("stop_proxy", no_args()).await
}

pub async fn get_traffic() -> Result<TrafficStats, String> {
    call::<TrafficStats>("get_traffic", no_args()).await
}

pub async fn get_proxy_status() -> Result<Option<ProxyStatus>, String> {
    call::<Option<ProxyStatus>>("get_proxy_status", no_args()).await
}

pub async fn update_config(
    relay_addr: Option<&str>,
    namespace: Option<&str>,
) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        relay_addr: Option<&'a str>,
        namespace: Option<&'a str>,
    }
    let args = serde_wasm_bindgen::to_value(&Args {
        relay_addr,
        namespace,
    })
    .map_err(|e| e.to_string())?;
    call_void("update_config", args).await
}

pub async fn join_network(name: &str, password: &str) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args<'a> {
        name: &'a str,
        password: &'a str,
    }
    let args =
        serde_wasm_bindgen::to_value(&Args { name, password }).map_err(|e| e.to_string())?;
    call_void("join_network", args).await
}

pub async fn leave_network() -> Result<(), String> {
    call_void("leave_network", no_args()).await
}

pub async fn set_system_proxy() -> Result<(), String> {
    call_void("set_system_proxy", no_args()).await
}

pub async fn clear_system_proxy() -> Result<(), String> {
    call_void("clear_system_proxy", no_args()).await
}
