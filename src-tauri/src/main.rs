// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::{Engine as _, engine::general_purpose};
use local_ip_address::local_ip;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri_plugin_store::StoreExt;

// ── Tray icons (embedded at compile time) ────────────────────────────────────

const TRAY_ICON_ON:  &[u8] = include_bytes!("../icons/tray-on-32.png");
const TRAY_ICON_OFF: &[u8] = include_bytes!("../icons/tray-off-32.png");

struct TrayState(tauri::tray::TrayIcon);

fn update_tray_icon(app: &AppHandle, any_enabled: bool) {
    let bytes = if any_enabled { TRAY_ICON_ON } else { TRAY_ICON_OFF };
    if let Ok(icon) = tauri::image::Image::from_bytes(bytes) {
        let tray = app.state::<TrayState>();
        let _ = tray.0.set_icon(Some(icon));
    }
}

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

// ── Constants ────────────────────────────────────────────────────────────────

const STORE_KEY: &str = "settings";
const REQUEST_TIMEOUT_SECS: u64 = 10;

// ── Data types ───────────────────────────────────────────────────────────────

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
    enabled: bool,          // device IP is in the alias
    online: bool,           // OPNsense gateway is up (not offline)
    gateway_status: String, // raw status: "online", "latency", "offline", etc.
    rtt: Option<String>,    // round-trip time (e.g. "12.3 ms")
    rttd: Option<String>,   // RTT deviation
    loss: Option<String>,   // packet loss (e.g. "0.0 %")
    error: Option<String>,
}

// ── App state ────────────────────────────────────────────────────────────────

struct AppState {
    settings: Mutex<Settings>,
    client: reqwest::Client,
    /// Last known enabled state per alias — used for accurate aggregate tray icon
    /// when a single gateway is toggled.
    alias_states: Mutex<HashMap<String, bool>>,
}

fn make_client() -> reqwest::Client {
    // danger_accept_invalid_certs: OPNsense commonly uses self-signed TLS certs.
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .expect("Failed to build HTTP client")
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Lock the settings mutex, recovering from poisoning.
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

fn get_local_ip() -> Result<String, String> {
    local_ip()
        .map(|ip| ip.to_string())
        .map_err(|e| format!("Failed to detect local IP: {}", e))
}

fn normalize_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

fn load_settings_from_store(app: &AppHandle) -> Settings {
    match app.store(".vpn-toggle.dat") {
        Ok(s) => {
            if let Some(val) = s.get(STORE_KEY) {
                serde_json::from_value(val).unwrap_or_default()
            } else {
                Settings::default()
            }
        }
        Err(e) => {
            write_log(&format!("Failed to open settings store: {}", e));
            Settings::default()
        }
    }
}

fn persist_settings(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let store = app.store(".vpn-toggle.dat")
        .map_err(|e| format!("Store error: {}", e))?;
    store.set(STORE_KEY, serde_json::to_value(settings).map_err(|e| e.to_string())?);
    store.save().map_err(|e| format!("Failed to save store: {}", e))?;
    Ok(())
}

// ── OPNsense API helpers ─────────────────────────────────────────────────────

/// Check if the device's local IP is in the firewall alias.
/// Uses GET /api/firewall/alias_util/list/{alias} — returns JSON with "rows" array.
/// `local_ip` is passed in to avoid redundant system calls when checking multiple gateways.
async fn fetch_alias_enabled(
    alias_name: &str,
    local_ip: &str,
    settings: &Settings,
    client: &reqwest::Client,
) -> Result<bool, String> {
    if settings.api_key.is_empty() || settings.api_secret.is_empty() {
        return Err("API credentials not configured".to_string());
    }

    let url = format!("{}/api/firewall/alias_util/list/{}", settings.base_url, alias_name);

    let response = client
        .get(&url)
        .header("Authorization", auth_header(settings))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() { "Connection to OPNsense timed out".to_string() }
            else { format!("Alias API request failed: {}", e) }
        })?;

    let http_status = response.status();
    let text = response.text().await.unwrap_or_default();

    if !http_status.is_success() {
        return Err(format!("Alias API error ({}): {}", http_status, text));
    }

    // Response: {"rows":[{"ip":"10.0.0.16","..."},...],"total":1}
    // Fall back to plain-text line scan if JSON parsing fails (older OPNsense).
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
        if let Some(rows) = json.get("rows").and_then(|v| v.as_array()) {
            return Ok(rows.iter().any(|row| {
                row.get("ip").and_then(|v| v.as_str()).map(|ip| ip.trim() == local_ip).unwrap_or(false)
                || row.as_str().map(|s| s.trim() == local_ip).unwrap_or(false)
            }));
        }
    }
    // Plain-text fallback
    Ok(text.lines().any(|line| line.trim() == local_ip))
}

#[derive(Debug)]
struct GatewayInfo {
    online: bool,
    status: String,
    rtt: Option<String>,
    rttd: Option<String>,
    loss: Option<String>,
}

/// Check OPNsense gateway status via the routes API.
async fn fetch_gateway_info(
    gateway_name: &str,
    settings: &Settings,
    client: &reqwest::Client,
) -> Result<GatewayInfo, String> {
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
            if e.is_timeout() { "Connection to OPNsense timed out".to_string() }
            else { format!("Gateway status API request failed: {}", e) }
        })?;

    let http_status = response.status();
    if !http_status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Gateway status API error ({}): {}", http_status, text));
    }

    let json: serde_json::Value = response.json().await
        .map_err(|e| format!("Failed to parse gateway status response: {}", e))?;

    if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
        for item in items {
            if item.get("name").and_then(|v| v.as_str()) == Some(gateway_name) {
                let gw_status = item.get("status").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                let is_online = !matches!(gw_status.as_str(), "offline" | "down" | "force_down");
                let rtt  = item.get("delay").and_then(|v| v.as_str()).map(|s| s.to_string());
                let rttd = item.get("stddev").and_then(|v| v.as_str()).map(|s| s.to_string());
                let loss = item.get("loss").and_then(|v| v.as_str()).map(|s| s.to_string());
                write_log(&format!("Gateway '{}' status='{}' online={} rtt={:?} loss={:?}",
                    gateway_name, gw_status, is_online, rtt, loss));
                return Ok(GatewayInfo { online: is_online, status: gw_status, rtt, rttd, loss });
            }
        }
        let names: Vec<&str> = items.iter()
            .filter_map(|i| i.get("name").and_then(|v| v.as_str()))
            .collect();
        let msg = format!("Gateway '{}' not found. Available: [{}]", gateway_name, names.join(", "));
        write_log(&msg);
        return Err(msg);
    }

    Err("Unexpected gateway status response format".to_string())
}

// ── Tauri commands ────────────────────────────────────────────────────────────

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
    let normalized = Settings {
        base_url: normalize_url(&settings.base_url),
        ..settings
    };
    persist_settings(&app, &normalized)?;
    *state.settings.lock().unwrap_or_else(|e| e.into_inner()) = normalized;
    Ok(())
}

#[tauri::command]
async fn toggle_vpn(
    app: AppHandle,
    alias_name: String,
    enable: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let settings = lock_settings(&state);

    if settings.api_key.is_empty() || settings.api_secret.is_empty() {
        return Err("API credentials not configured".to_string());
    }

    let local_ip = get_local_ip()?;
    let endpoint = if enable { "add" } else { "delete" };
    let url = format!("{}/api/firewall/alias_util/{}/{}", settings.base_url, endpoint, alias_name);

    let mut body = HashMap::new();
    body.insert("address", local_ip.as_str());

    let response = state.client
        .post(&url)
        .header("Authorization", auth_header(&settings))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() { "Connection to OPNsense timed out".to_string() }
            else { format!("API request failed: {}", e) }
        })?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Failed to update alias: {}", text));
    }

    // Apply firewall changes
    // OPNsense requires Content-Length to be set (411 otherwise) — send empty body.
    let reconfigure_url = format!("{}/api/firewall/alias/reconfigure", settings.base_url);
    let reconfigure_response = state.client
        .post(&reconfigure_url)
        .header("Authorization", auth_header(&settings))
        .header("Content-Length", "0")
        .body("")
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() { "Alias updated but reconfigure timed out — changes may not be applied yet".to_string() }
            else { format!("Reconfigure request failed: {}", e) }
        })?;

    if !reconfigure_response.status().is_success() {
        let text = reconfigure_response.text().await.unwrap_or_default();
        return Err(format!("Failed to reconfigure firewall: {}", text));
    }

    // Update alias state cache and compute accurate aggregate for tray icon.
    // This prevents a false red icon when disabling one gateway while another is still active.
    let any_enabled = {
        let mut states = state.alias_states.lock().unwrap_or_else(|e| e.into_inner());
        states.insert(alias_name.clone(), enable);
        states.values().any(|&v| v)
    };
    update_tray_icon(&app, any_enabled);

    Ok(())
}

#[tauri::command]
async fn get_all_vpn_status(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<VpnStatus>, String> {
    let settings = lock_settings(&state);
    let client = &state.client;

    // Resolve local IP once — reused for all alias checks this cycle.
    let local_ip = get_local_ip()?;

    let mut statuses = Vec::new();

    for gateway in &settings.gateways {
        let (enabled_result, gw_result) = tokio::join!(
            fetch_alias_enabled(&gateway.alias_name, &local_ip, &settings, client),
            fetch_gateway_info(&gateway.gateway_name, &settings, client)
        );

        let enabled = enabled_result.as_ref().copied().unwrap_or(false);
        let (online, gw_status, rtt, rttd, loss) = match &gw_result {
            Ok(info) => (info.online, info.status.clone(), info.rtt.clone(), info.rttd.clone(), info.loss.clone()),
            Err(_)   => (false, "unknown".to_string(), None, None, None),
        };

        let error = match (enabled_result.err(), gw_result.err()) {
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
            gateway_status: gw_status,
            rtt,
            rttd,
            loss,
            error,
        });
    }

    // Refresh alias state cache and sync tray icon.
    {
        let mut states = state.alias_states.lock().unwrap_or_else(|e| e.into_inner());
        for s in &statuses {
            states.insert(s.alias_name.clone(), s.enabled);
        }
    }
    let any_enabled = statuses.iter().any(|s| s.enabled);
    update_tray_icon(&app, any_enabled);

    Ok(statuses)
}

// ── Main ─────────────────────────────────────────────────────────────────────

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
                alias_states: Mutex::new(HashMap::new()),
            });

            let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            write_log("Setup: building tray icon");
            let tray_icon = TrayIconBuilder::new()
                .icon(tauri::image::Image::from_bytes(TRAY_ICON_OFF).expect("tray-off icon"))
                .menu(&menu)
                .on_menu_event(|app: &AppHandle, event| match event.id().as_ref() {
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
                .on_tray_icon_event(|tray: &tauri::tray::TrayIcon, event| {
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
            app.manage(TrayState(tray_icon));

            // Minimize to tray: hide window on minimize (removes from taskbar).
            // Close button exits normally.
            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Resized(_) = event {
                        if window_clone.is_minimized().unwrap_or(false) {
                            let _ = window_clone.hide();
                        }
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
