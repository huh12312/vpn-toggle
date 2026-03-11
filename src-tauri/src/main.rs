// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::{Engine as _, engine::general_purpose};
use local_ip_address::local_ip;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri_plugin_store::StoreExt;
use std::sync::Mutex;

fn get_local_ip() -> Result<String, String> {
    local_ip()
        .map(|ip| ip.to_string())
        .map_err(|e| format!("Failed to detect local IP: {}", e))
}

const STORE_KEY: &str = "settings";

#[derive(Debug, Serialize, Deserialize, Clone)]
struct VpnGateway {
    display_name: String,
    gateway_name: String, // OPNsense gateway name for status checks (e.g. WAN_VPN)
    alias_name: String,   // Firewall alias name for toggle (e.g. vpn_devices)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Settings {
    base_url: String,
    api_key: String,
    api_secret: String,
    gateways: Vec<VpnGateway>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            base_url: "https://10.0.0.1:444".to_string(),
            api_key: String::new(),
            api_secret: String::new(),
            gateways: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize)]
struct VpnStatus {
    gateway_name: String,
    alias_name: String,
    display_name: String,
    enabled: bool,      // true = device IP is in the alias (routing through VPN)
    online: bool,       // true = OPNsense gateway is up
    error: Option<String>,
}

struct AppState {
    settings: Mutex<Settings>,
}

fn load_settings_from_store(app: &AppHandle) -> Settings {
    let store = app.store(".vpn-toggle.dat");
    match store {
        Ok(s) => {
            if let Some(val) = s.get(STORE_KEY) {
                serde_json::from_value(val).unwrap_or_default()
            } else {
                Settings::default()
            }
        }
        Err(_) => Settings::default(),
    }
}

fn persist_settings(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let store = app.store(".vpn-toggle.dat").map_err(|e| format!("Store error: {}", e))?;
    store.set(STORE_KEY, serde_json::to_value(settings).map_err(|e| e.to_string())?);
    store.save().map_err(|e| format!("Failed to save store: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let settings = state.settings.lock().unwrap();
    Ok(settings.clone())
}

#[tauri::command]
async fn save_settings(
    settings: Settings,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    persist_settings(&app, &settings)?;
    let mut app_settings = state.settings.lock().unwrap();
    *app_settings = settings;
    Ok(())
}

fn make_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))
}

fn auth_header(settings: &Settings) -> String {
    format!(
        "Basic {}",
        general_purpose::STANDARD.encode(format!("{}:{}", settings.api_key, settings.api_secret))
    )
}

// Check if the device's local IP is in the firewall alias (toggle state)
async fn fetch_alias_enabled(alias_name: &str, settings: &Settings) -> Result<bool, String> {
    if settings.api_key.is_empty() || settings.api_secret.is_empty() {
        return Err("API credentials not configured".to_string());
    }

    let client = make_client()?;
    let url = format!("{}/api/firewall/alias/getAliasContent/{}", settings.base_url, alias_name);

    let response = client
        .get(&url)
        .header("Authorization", auth_header(settings))
        .send()
        .await
        .map_err(|e| format!("Alias API request failed: {}", e))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(format!("Alias API error ({}): {}", status, text));
    }

    let local_ip = get_local_ip()?;
    Ok(text.contains(&local_ip))
}

// Check OPNsense gateway status (online/offline) via the routes API
async fn fetch_gateway_online(gateway_name: &str, settings: &Settings) -> Result<bool, String> {
    if settings.api_key.is_empty() || settings.api_secret.is_empty() {
        return Err("API credentials not configured".to_string());
    }

    let client = make_client()?;
    let url = format!("{}/api/routes/gateway/status", settings.base_url);

    let response = client
        .get(&url)
        .header("Authorization", auth_header(settings))
        .send()
        .await
        .map_err(|e| format!("Gateway status API request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Gateway status API error ({}): {}", status, text));
    }

    let json: serde_json::Value = response.json().await
        .map_err(|e| format!("Failed to parse gateway status response: {}", e))?;

    if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
        for item in items {
            if item.get("name").and_then(|v| v.as_str()) == Some(gateway_name) {
                let gw_status = item.get("status").and_then(|v| v.as_str()).unwrap_or("");
                return Ok(gw_status == "online");
            }
        }
        return Err(format!("Gateway '{}' not found in OPNsense", gateway_name));
    }

    Err("Unexpected gateway status response format".to_string())
}

#[tauri::command]
async fn check_vpn_status(
    alias_name: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let settings = state.settings.lock().unwrap().clone();
    fetch_alias_enabled(&alias_name, &settings).await
}

#[tauri::command]
async fn toggle_vpn(
    alias_name: String,
    enable: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let settings = state.settings.lock().unwrap().clone();

    if settings.api_key.is_empty() || settings.api_secret.is_empty() {
        return Err("API credentials not configured".to_string());
    }

    let client = make_client()?;
    let local_ip = get_local_ip()?;
    let endpoint = if enable { "addAliasAddress" } else { "delAliasAddress" };
    let url = format!("{}/api/firewall/alias/{}/{}", settings.base_url, endpoint, alias_name);

    let mut body = std::collections::HashMap::new();
    body.insert("address", local_ip.as_str());

    let response = client
        .post(&url)
        .header("Authorization", auth_header(&settings))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Failed to update alias: {}", text));
    }

    // Apply changes
    let reconfigure_url = format!("{}/api/firewall/alias/reconfigure", settings.base_url);
    let reconfigure_response = client
        .post(&reconfigure_url)
        .header("Authorization", auth_header(&settings))
        .send()
        .await
        .map_err(|e| format!("API reconfigure request failed: {}", e))?;

    if !reconfigure_response.status().is_success() {
        let text = reconfigure_response.text().await.unwrap_or_default();
        return Err(format!("Failed to reconfigure firewall: {}", text));
    }

    Ok(())
}

#[tauri::command]
async fn get_all_vpn_status(state: State<'_, AppState>) -> Result<Vec<VpnStatus>, String> {
    let settings = state.settings.lock().unwrap().clone();
    let mut statuses = Vec::new();

    for gateway in &settings.gateways {
        let enabled_result = fetch_alias_enabled(&gateway.alias_name, &settings).await;
        let online_result = fetch_gateway_online(&gateway.gateway_name, &settings).await;

        let (enabled, online, error) = match (enabled_result, online_result) {
            (Ok(e), Ok(o)) => (e, o, None),
            (Err(e), _) => (false, false, Some(e)),
            (_, Err(e)) => (false, false, Some(e)),
        };

        statuses.push(VpnStatus {
            gateway_name: gateway.gateway_name.clone(),
            alias_name: gateway.alias_name.clone(),
            display_name: gateway.display_name.clone(),
            enabled,
            online,
            error,
        });
    }

    Ok(statuses)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            // Load persisted settings into AppState
            let settings = load_settings_from_store(app.handle());
            app.manage(AppState {
                settings: Mutex::new(settings),
            });

            // Tray menu
            let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))
                .map_err(|e| format!("Failed to load tray icon: {}", e))?;

            let _tray = TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        ..
                    } = event
                    {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Hide to tray on close instead of quitting
            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_clone.hide();
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            check_vpn_status,
            toggle_vpn,
            get_all_vpn_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
