// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::{Engine as _, engine::general_purpose};
use keyring::Entry;
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

const LOG_MAX_BYTES: u64 = 1_000_000; // 1 MB — rotate when exceeded

fn write_log(msg: &str) {
    use std::io::Write;
    if let Some(path) = log_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // Rotate: if log exceeds 1 MB, rename to app.log.1 and start fresh.
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() >= LOG_MAX_BYTES {
                let backup = path.with_extension("log.1");
                let _ = std::fs::rename(&path, &backup);
            }
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
    gateways: Vec<VpnGateway>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            base_url: "https://10.0.0.1:444".to_string(),
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
    /// Cached credentials loaded from OS keyring at startup.
    /// Updated on save_credentials / cleared on delete_credentials.
    /// Avoids hitting the keyring on every API call (important on Linux).
    credentials: Mutex<Option<(String, String)>>,
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

/// Load credentials from OS keyring. Returns None if either credential is missing.
/// Note: On macOS/Linux, this requires the app to run in a context with a running secret service.
fn load_credentials_from_keyring() -> Option<(String, String)> {
    let api_key_entry = Entry::new("vpn-toggle", "api_key").ok()?;
    let api_secret_entry = Entry::new("vpn-toggle", "api_secret").ok()?;

    let api_key = api_key_entry.get_password().ok()?;
    let api_secret = api_secret_entry.get_password().ok()?;

    Some((api_key, api_secret))
}

fn auth_header(api_key: &str, api_secret: &str) -> String {
    format!(
        "Basic {}",
        general_purpose::STANDARD.encode(format!("{}:{}", api_key, api_secret))
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
    api_key: &str,
    api_secret: &str,
    client: &reqwest::Client,
) -> Result<bool, String> {
    if api_key.is_empty() || api_secret.is_empty() {
        return Err("API credentials not configured".to_string());
    }

    let url = format!("{}/api/firewall/alias_util/list/{}", settings.base_url, alias_name);

    let response = client
        .get(&url)
        .header("Authorization", auth_header(api_key, api_secret))
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
    api_key: &str,
    api_secret: &str,
    client: &reqwest::Client,
) -> Result<GatewayInfo, String> {
    if api_key.is_empty() || api_secret.is_empty() {
        return Err("API credentials not configured".to_string());
    }

    let url = format!("{}/api/routes/gateway/status", settings.base_url);

    let response = client
        .get(&url)
        .header("Authorization", auth_header(api_key, api_secret))
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
async fn save_credentials(
    api_key: String,
    api_secret: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if api_key.trim().is_empty() {
        return Err("API key cannot be empty".to_string());
    }
    if api_secret.trim().is_empty() {
        return Err("API secret cannot be empty".to_string());
    }

    let api_key_entry = Entry::new("vpn-toggle", "api_key")
        .map_err(|e| format!("Failed to create keyring entry for api_key: {}", e))?;
    let api_secret_entry = Entry::new("vpn-toggle", "api_secret")
        .map_err(|e| format!("Failed to create keyring entry for api_secret: {}", e))?;

    api_key_entry.set_password(&api_key)
        .map_err(|e| format!("Failed to save api_key to keyring: {}", e))?;
    api_secret_entry.set_password(&api_secret)
        .map_err(|e| format!("Failed to save api_secret to keyring: {}", e))?;

    // Update in-memory cache so subsequent API calls don't re-hit the keyring.
    *state.credentials.lock().unwrap_or_else(|e| e.into_inner()) = Some((api_key, api_secret));

    Ok(())
}

#[tauri::command]
async fn load_credentials(state: State<'_, AppState>) -> Result<Option<(String, String)>, String> {
    Ok(state.credentials.lock().unwrap_or_else(|e| e.into_inner()).clone())
}

#[tauri::command]
async fn delete_credentials(state: State<'_, AppState>) -> Result<(), String> {
    let api_key_entry = Entry::new("vpn-toggle", "api_key")
        .map_err(|e| format!("Failed to create keyring entry for api_key: {}", e))?;
    let api_secret_entry = Entry::new("vpn-toggle", "api_secret")
        .map_err(|e| format!("Failed to create keyring entry for api_secret: {}", e))?;

    // Ignore errors if credentials don't already exist
    let _ = api_key_entry.delete_credential();
    let _ = api_secret_entry.delete_credential();

    // Clear in-memory cache.
    *state.credentials.lock().unwrap_or_else(|e| e.into_inner()) = None;

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

    let (api_key, api_secret) = state.credentials
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or_else(|| "API credentials not configured".to_string())?;

    let local_ip = get_local_ip()?;
    let endpoint = if enable { "add" } else { "delete" };
    let url = format!("{}/api/firewall/alias_util/{}/{}", settings.base_url, endpoint, alias_name);

    let mut body = HashMap::new();
    body.insert("address", local_ip.as_str());

    let response = state.client
        .post(&url)
        .header("Authorization", auth_header(&api_key, &api_secret))
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
        .header("Authorization", auth_header(&api_key, &api_secret))
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

    let (api_key, api_secret) = state.credentials
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or_else(|| "API credentials not configured".to_string())?;

    // Resolve local IP once — reused for all alias checks this cycle.
    let local_ip = get_local_ip()?;

    let mut statuses = Vec::new();

    for gateway in &settings.gateways {
        let (enabled_result, gw_result) = tokio::join!(
            fetch_alias_enabled(&gateway.alias_name, &local_ip, &settings, &api_key, &api_secret, client),
            fetch_gateway_info(&gateway.gateway_name, &settings, &api_key, &api_secret, client)
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
            let cached_credentials = load_credentials_from_keyring();
            if cached_credentials.is_some() {
                write_log("Setup: credentials loaded from keyring");
            } else {
                write_log("Setup: no credentials found in keyring");
            }
            app.manage(AppState {
                settings: Mutex::new(settings),
                client: make_client(),
                credentials: Mutex::new(cached_credentials),
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

            // Pre-populate alias_states by fetching alias membership for all configured
            // gateways in the background. This ensures the tray icon reflects the true
            // VPN state immediately, even before the first manual refresh or toggle.
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state: State<'_, AppState> = app_handle.state();
                    let settings = lock_settings(&state);
                    let creds = state.credentials.lock().unwrap_or_else(|e| e.into_inner()).clone();

                    if let Some((api_key, api_secret)) = creds {
                        if let Ok(local_ip) = get_local_ip() {
                            let mut states = HashMap::new();
                            for gateway in &settings.gateways {
                                if let Ok(enabled) = fetch_alias_enabled(
                                    &gateway.alias_name, &local_ip, &settings,
                                    &api_key, &api_secret, &state.client,
                                ).await {
                                    states.insert(gateway.alias_name.clone(), enabled);
                                }
                            }
                            let any_enabled = states.values().any(|&v| v);
                            *state.alias_states.lock().unwrap_or_else(|e| e.into_inner()) = states;
                            update_tray_icon(&app_handle, any_enabled);
                            write_log("Setup: alias_states pre-populated from OPNsense");
                        }
                    }
                });
            }

            // First-run: show window automatically if no credentials are saved yet,
            // so the user can configure the app. Otherwise stay hidden in tray.
            {
                let has_credentials = app.state::<AppState>()
                    .credentials.lock().unwrap_or_else(|e| e.into_inner()).is_some();
                if !has_credentials {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                        write_log("Setup: first run — showing window for initial configuration");
                    }
                }
            }

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
            save_credentials,
            load_credentials,
            delete_credentials,
            toggle_vpn,
            get_all_vpn_status
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            write_log(&format!("[FATAL] Tauri runtime error: {}", e));
        });
}
