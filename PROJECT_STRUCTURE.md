# VPN Toggle - Project Structure

## Complete File Tree

```
vpn-toggle/
├── package.json                 # Node.js dependencies and scripts
├── vite.config.ts              # Vite bundler configuration
├── tsconfig.json               # TypeScript configuration
├── tsconfig.node.json          # TypeScript config for Node files
├── tailwind.config.js          # Tailwind CSS configuration
├── postcss.config.js           # PostCSS configuration
├── index.html                  # HTML entry point
├── .gitignore                  # Git ignore rules
├── README.md                   # Main documentation
├── QUICKSTART.md               # Quick start guide
├── PROJECT_STRUCTURE.md        # This file
├── create-icons.py             # Icon generator script (Python)
├── create-icons.sh             # Icon generator script (Bash)
│
├── src/                        # React frontend source
│   ├── main.tsx               # React app entry point
│   ├── index.css              # Global styles with Tailwind
│   ├── App.tsx                # Main app component
│   └── components/
│       ├── VpnList.tsx        # VPN gateway list and toggles
│       └── Settings.tsx       # Settings panel component
│
└── src-tauri/                  # Rust/Tauri backend
    ├── Cargo.toml             # Rust dependencies
    ├── build.rs               # Tauri build script
    ├── tauri.conf.json        # Tauri app configuration
    ├── icons/                 # Application icons
    │   ├── 32x32.png
    │   ├── 128x128.png
    │   ├── 128x128@2x.png
    │   ├── icon.ico           # Windows icon
    │   └── icon.icns          # macOS icon
    └── src/
        └── main.rs            # Rust backend with Tauri commands
```

## Key Components

### Frontend (React + TypeScript)

**src/App.tsx**
- Main application component
- Manages settings state
- Switches between VPN list and settings views

**src/components/VpnList.tsx**
- Displays all configured VPN gateways
- Shows status indicators (connected/disconnected/error)
- Toggle switches for each gateway
- Refresh button to update statuses

**src/components/Settings.tsx**
- OPNsense configuration form
- API credentials input
- Gateway management (add/remove/edit)
- Save/cancel functionality

### Backend (Rust + Tauri)

**src-tauri/src/main.rs**
- Tauri commands for OPNsense API integration
- System tray implementation
- Window event handlers (hide to tray on close)
- State management for settings

### Tauri Commands

1. `get_settings()` - Load saved settings
2. `save_settings(settings)` - Persist settings
3. `check_vpn_status(gateway_name)` - Check if VPN is enabled
4. `toggle_vpn(gateway_name, enable)` - Toggle VPN on/off
5. `get_all_vpn_status()` - Get status for all gateways

### OPNsense API Integration

The app uses these OPNsense API endpoints:

- `GET /api/firewall/alias/getAliasContent/{alias}` - Check current alias content
- `POST /api/firewall/alias/addAliasAddress/{alias}` - Add route to alias
- `POST /api/firewall/alias/delAliasAddress/{alias}` - Remove route from alias
- `POST /api/firewall/alias/reconfigure` - Apply firewall changes

All requests use HTTP Basic Auth with API key:secret and skip TLS verification for self-signed certs.

## Build Outputs

After running `npm run tauri build`:

```
src-tauri/target/release/
├── vpn-toggle.exe              # Standalone executable (Windows)
└── bundle/
    └── nsis/                   # NSIS installer files
        └── vpn-toggle_1.0.0_x64-setup.exe
```

## Configuration Files

### package.json
- npm scripts: `dev`, `build`, `preview`, `tauri`
- Dependencies: React, Tauri plugins
- Dev dependencies: TypeScript, Vite, Tailwind

### tauri.conf.json
- App identifier: `com.vpntoggle.app`
- Window configuration (size, title, etc.)
- Bundle settings (NSIS for Windows)
- Tray icon configuration
- Plugin settings (store)

### Cargo.toml
- Tauri 2.0 dependencies
- reqwest (HTTP client)
- serde/serde_json (serialization)
- base64 (auth encoding)
- tauri-plugin-store (persistent storage)

## Data Flow

1. **User toggles VPN**
   - Frontend calls `toggle_vpn()` Tauri command
   - Rust backend makes OPNsense API calls
   - Adds/removes `0.0.0.0/0` from firewall alias
   - Reconfigures firewall
   - Frontend refreshes status

2. **Settings persistence**
   - Settings saved via `save_settings()` command
   - Stored in app data directory (`.vpn-toggle.dat`)
   - Loaded on app startup

3. **System tray**
   - App lives in system tray
   - Closing window hides to tray (doesn't quit)
   - Left-click tray icon to show window
   - Right-click for menu (Show/Quit)

## Development Workflow

1. Install dependencies: `npm install`
2. Run dev mode: `npm run tauri dev`
3. Make changes (hot reload for frontend)
4. Build: `npm run tauri build`
5. Test executable in `src-tauri/target/release/`

## Security Notes

- TLS certificate verification is disabled (self-signed certs)
- API credentials stored locally in encrypted format
- No auto-start on boot (user must launch manually)
- Basic Auth used for OPNsense API (key:secret)
