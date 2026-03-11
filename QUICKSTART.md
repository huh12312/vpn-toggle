# Quick Start Guide

## Installation

1. **Install dependencies:**
   ```bash
   npm install
   ```

2. **Development mode:**
   ```bash
   npm run tauri dev
   ```

3. **Build for production:**
   ```bash
   npm run tauri build
   ```

   The executable will be in `src-tauri/target/release/vpn-toggle.exe`

## First-Time Setup

1. Launch the app - it will show the Settings screen automatically
2. Configure your OPNsense connection:
   - **Base URL**: Your OPNsense router URL (e.g., `https://10.0.0.1:444`)
   - **API Key**: From System > Access > Users in OPNsense
   - **API Secret**: From the same location

3. Add VPN gateways:
   - Click "Add Gateway"
   - **Display Name**: Any name you want (e.g., "US East VPN")
   - **Gateway Name**: The exact alias name in OPNsense (e.g., `vpn_us_east`)

4. Click "Save Settings"

## Usage

- **Toggle VPN**: Click the toggle switch next to any gateway
- **Refresh Status**: Click the "Refresh" button to update all statuses
- **System Tray**:
  - Close the window to minimize to system tray
  - Left-click tray icon to show window
  - Right-click tray icon for menu

## OPNsense Configuration

The app expects your VPN gateways to be configured as firewall aliases in OPNsense:

1. Go to Firewall > Aliases
2. Create aliases for your VPN gateways
3. Use those alias names in the app's gateway configuration

**How toggles work:**
- VPN is "ON" when the alias contains `0.0.0.0/0`
- VPN is "OFF" when `0.0.0.0/0` is not in the alias
- The app adds/removes this route and reconfigures the firewall

## Troubleshooting

### "API credentials not configured"
- Open Settings and ensure API Key and Secret are filled in

### "API request failed: certificate verify failed"
- This is expected for self-signed certs - the app automatically skips cert verification

### "Failed to update alias"
- Verify the gateway name matches exactly with your OPNsense alias name
- Check that your API credentials have permission to modify firewall aliases

### Toggle doesn't change state
- Click Refresh to ensure you're seeing the current state
- Check OPNsense logs for API access errors
- Verify network connectivity to your OPNsense router

## Building for Distribution

To create a standalone executable:

```bash
npm run tauri build
```

The output files will be in:
- Windows: `src-tauri/target/release/vpn-toggle.exe`
- NSIS Installer: `src-tauri/target/release/bundle/nsis/`

## Customization

### Change Icons
Replace the placeholder icons in `src-tauri/icons/` with your own:
- `icon.ico` - Windows icon
- `icon.icns` - macOS icon (if needed)
- `32x32.png`, `128x128.png`, `128x128@2x.png` - Various sizes

Or use Tauri's icon generator:
```bash
npm run tauri icon path/to/your-icon.png
```

### Default Base URL
Edit `src-tauri/src/main.rs` line 28 to change the default base URL:
```rust
base_url: "https://YOUR-ROUTER-IP:444".to_string(),
```

### Window Size
Edit `src-tauri/tauri.conf.json` under `app.windows[0]`:
```json
{
  "width": 500,
  "height": 600
}
```
