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
use std::time::Duration;

// ── Startup logging ──────────────────────────────────────────────────────────

fn log_path() -> Option<std::path::PathBuf> {
    std::env::var("APPDATA").ok().map(|appdata| {
        std::path::PathBuf::from(appdata)
            .join("VPN Toggle")
            .join("app.log")
    })
}

fn write_log(msg: &str) {
    use std::io::Write;
    if let Some(path) = log_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = writeln!(file, "[{}] {}", ts, msg);
        }
    }
}

fn setup_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        write_log(&format!("[PANIC] {}", info));
    }));
}

const STORE_KEY: &str = "settings";
const REQUEST_TIMEOUT_SECS: u64 = 10;

fn get_local_ip() -> Result<String, String> {
    local_ip()
        .map(|ip| ip.to_string())
        .map_err(|e| format!("Failed to detect local IP: {}", e))
}

fn normalize_url(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct VpnGateway {
    display_name: String,
    gateway_name: String, // OPNsense gateway name for status checks (e.g. WAN_VPN)
    #[serde(default)]
    alias_name: String,   // Firewall alias name for toggle — serde(default) handles schema upgrades
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
    enabled: bool,   // device IP is in the alias
    online: bool,    // OPNsense gateway is up
    error: Option<String>,
}

struct AppState {
    settings: Mutex<Settings>,
    client: reqwest::Client, // shared, connection-pooled
}

fn make_client() -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .expect("Failed to build HTTP client")
}

/// Lock the settings mutex, recovering from poisoning
fn lock_settings(state: &State<'_, AppState>) -> Settings {
    state.settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

fn auth_header(settings: &Settings) -> String {
    format!(
        "Basic {}",
        general_purpose::STANDARD.encode(format!("{}:{}", settings.api_key, settings.api_secret))
    )
}

fn load_settings_from_store(app: &AppHandle) -> Settings {
    let store = app.store(".vpn-toggle.dat");
    match store {
        Ok(s) => {
            if let Some(val) = s.get(STORE_KEY) {
                // serde(default) on VpnGateway::alias_name handles old schema gracefully
                serde_json::from_value(val).unwrap_or_default()
            } else {
                Settings::default()
            }
        }
        Err(_) => Settings::default(),
    }
}

fn persist_settings(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let store = app.store(".vpn-toggle.dat")
        .map_err(|e| format!("Store error: {}", e))?;
    store.set(STORE_KEY, serde_json::to_value(settings).map_err(|e| e.to_string())?);
    store.save().map_err(|e| format!("Failed to save store: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok(lock_settings(&state))
}

#[tauri::command]
async fn save_settings(
    settings: Settings,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    // Normalize base URL — strip trailing slash to prevent double-slash in API paths
    let normalized = Settings {
        base_url: normalize_url(&settings.base_url),
        ..settings
    };
    persist_settings(&app, &normalized)?;
    *state.settings.lock().unwrap_or_else(|e| e.into_inner()) = normalized;
    Ok(())
}

/// Check if the device's local IP is in the firewall alias (toggle state).
/// Uses exact line match to avoid substring false positives (e.g. 192.168.1.1 vs 192.168.1.10).
async fn fetch_alias_enabled(
    alias_name: &str,
    settings: &Settings,
    client: &reqwest::Client,
) -> Result<bool, String> {
    if settings.api_key.is_empty() || settings.api_secret.is_empty() {
        return Err("API credentials not configured".to_string());
    }

    let url = format!("{}/api/firewall/alias/getAliasContent/{}", settings.base_url, alias_name);

    let response = client
        .get(&url)
        .header("Authorization", auth_header(settings))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "Connection to OPNsense timed out".to_string()
            } else {
                format!("Alias API request failed: {}", e)
            }
        })?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(format!("Alias API error ({}): {}", status, text));
    }

    let local_ip = get_local_ip()?;
    Ok(text.lines().any(|line| line.trim() == local_ip))
}

/// Check OPNsense gateway status (online/offline) via the routes API.
async fn fetch_gateway_online(
    gateway_name: &str,
    settings: &Settings,
    client: &reqwest::Client,
) -> Result<bool, String> {
    if settings.api_key.is_empty() || settings.api_secret.is_empty() {
        return Err("API credentials not configured".to_string());
    }

    let url = format!("{}/api/routes/gateway/status", settings.base_url);

    let response = client
        .get(&url)
        .header("Authorization", auth_header(settings))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "Connection to OPNsense timed out".to_string()
            } else {
                format!("Gateway status API request failed: {}", e)
            }
        })?;

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
async fn toggle_vpn(
    alias_name: String,
    enable: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let settings = lock_settings(&state);

    if settings.api_key.is_empty() || settings.api_secret.is_empty() {
        return Err("API credentials not configured".to_string());
    }

    let local_ip = get_local_ip()?;
    let endpoint = if enable { "addAliasAddress" } else { "delAliasAddress" };
    let url = format!("{}/api/firewall/alias/{}/{}", settings.base_url, endpoint, alias_name);

    let mut body = std::collections::HashMap::new();
    body.insert("address", local_ip.as_str());

    let response = state.client
        .post(&url)
        .header("Authorization", auth_header(&settings))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "Connection to OPNsense timed out".to_string()
            } else {
                format!("API request failed: {}", e)
            }
        })?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Failed to update alias: {}", text));
    }

    // Apply firewall changes
    let reconfigure_url = format!("{}/api/firewall/alias/reconfigure", settings.base_url);
    let reconfigure_response = state.client
        .post(&reconfigure_url)
        .header("Authorization", auth_header(&settings))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "Alias updated but reconfigure timed out — changes may not be applied yet".to_string()
            } else {
                format!("Reconfigure request failed: {}", e)
            }
        })?;

    if !reconfigure_response.status().is_success() {
        let text = reconfigure_response.text().await.unwrap_or_default();
        return Err(format!("Failed to reconfigure firewall: {}", text));
    }

    Ok(())
}

#[tauri::command]
async fn get_all_vpn_status(state: State<'_, AppState>) -> Result<Vec<VpnStatus>, String> {
    let settings = lock_settings(&state);
    let client = &state.client;
    let mut statuses = Vec::new();

    for gateway in &settings.gateways {
        // Run alias and gateway status checks in parallel
        let (enabled_result, online_result) = tokio::join!(
            fetch_alias_enabled(&gateway.alias_name, &settings, client),
            fetch_gateway_online(&gateway.gateway_name, &settings, client)
        );

        let enabled = enabled_result.as_ref().copied().unwrap_or(false);
        let online = online_result.as_ref().copied().unwrap_or(false);

        // Report both errors if both fail
        let error = match (enabled_result.err(), online_result.err()) {
            (None, None) => None,
            (Some(e), None) => Some(format!("Alias: {}", e)),
            (None, Some(e)) => Some(format!("Gateway: {}", e)),
            (Some(e1), Some(e2)) => Some(format!("Alias: {} | Gateway: {}", e1, e2)),
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
    setup_panic_hook();
    write_log("Application starting");

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            write_log("Setup: loading settings");
            let settings = load_settings_from_store(app.handle());
            app.manage(AppState {
                settings: Mutex::new(settings),
                client: make_client(),
            });

            let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            write_log("Setup: building tray icon");
            let _tray = TrayIconBuilder::new()
                .icon(tauri::include_image!("icons/32x32.png"))
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

            write_log("Setup: complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            toggle_vpn,
            get_all_vpn_status
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            write_log(&format!("[FATAL] Tauri runtime error: {}", e));
        });
}
