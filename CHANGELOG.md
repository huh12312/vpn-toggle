# Changelog

All notable changes to VPN Toggle are documented here.

## [1.1.14] — 2026-03-13

### Security
- **Keyring credential storage** — API key and API secret are now stored in the OS native secret store (Windows Credential Manager / macOS Keychain / Linux Secret Service) instead of plain JSON on disk
- **API key field masked** — both API key and API secret fields are now password-type inputs with a shared show/hide toggle
- **Credential validation** — `save_credentials` now rejects empty strings before writing to the keyring

### Added
- **Clear credentials** — Settings now has a "Clear saved credentials…" button with inline confirmation
- **Credential cache** — credentials are loaded from keyring once at startup and cached in app state; avoids hitting the keyring on every API call (important on Linux)
- **Startup alias pre-fetch** — alias states are fetched in the background on launch so the tray icon is accurate from the first second, not just after the first manual refresh
- **First-run auto-show** — window opens automatically on first launch (no credentials configured); stays in tray on subsequent starts

### Fixed
- **Log rotation** — app.log now rotates to app.log.1 at 1 MB; log no longer grows unboundedly
- **`useCallback` deps** — VpnList now depends on the full `settings` object; renaming a gateway without changing count or URL now correctly triggers a refresh
- **Inline error handling** — load failures show a retry screen; save failures show an inline banner; no more `alert()` / `window.confirm()` native dialogs
- **Bundle identifier** — changed from `com.vpntoggle.app` (conflicts with macOS `.app` extension) to `com.vpntoggle.desktop`

### CI
- **Lint job** — `tsc --noEmit` + `cargo check` now run before the full Tauri build, catching type and compile errors in ~1 minute instead of ~13
- **Node 22** — upgraded from Node 20 (deprecated June 2026) to Node 22 LTS
- **Multiline regex** — fixed PowerShell version-bump regex to use `(?m)` flag so `^` matches line-start correctly

## [1.1.13] — 2026-03-12

### Changed
- Code review cleanup: CSP hardening, minimum window size, import order, URL whitespace normalization, removed unused deps

## [1.1.x] — 2026-03-12

### Fixed
- Store `TrayIcon` in managed state instead of ID lookup (Tauri 2.10 API change)
- Add `image-png` feature flag to enable `Image::from_bytes` for dynamic tray icon
- Explicit closure type annotations for tray builder

## [1.1.0] — 2026-03-12

### Added
- **Dynamic tray icon** — green when any VPN is active, red when all are off
- **Alias state cache** — tracks per-gateway state for accurate aggregate tray icon when toggling individual gateways

## [1.0.0] — Initial Release

### Added
- System tray app with minimize-to-tray behavior
- Multiple VPN gateway support with per-gateway configuration (Display Name, Gateway Name, Alias Name)
- Gateway status via OPNsense routes API (`/api/routes/gateway/status`)
- VPN toggle via firewall alias add/remove (`/api/firewall/alias_util/add|delete`)
- Auto-refresh every 30 seconds
- RTT / packet loss display per gateway
- Settings UI with validation
- NSIS Windows installer via GitHub Actions on `v*` tags
