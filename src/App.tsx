import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import VpnList from "./components/VpnList";
import Settings from "./components/Settings";

export interface VpnGateway {
  display_name: string;
  gateway_name: string;
  alias_name: string;
}

export interface AppSettings {
  base_url: string;
  gateways: VpnGateway[];
}

export interface Credentials {
  api_key: string;
  api_secret: string;
}

function App() {
  const [showSettings, setShowSettings] = useState(false);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  // undefined = not yet fetched; null = fetched but none stored; Credentials = loaded
  const [credentials, setCredentials] = useState<Credentials | null | undefined>(undefined);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  useEffect(() => {
    loadAll();
  }, []);

  const loadAll = async () => {
    setLoadError(null);
    try {
      const [s, creds] = await Promise.all([
        invoke<AppSettings>("get_settings"),
        invoke<[string, string] | null>("load_credentials"),
      ]);
      setSettings(s);
      if (creds) {
        setCredentials({ api_key: creds[0], api_secret: creds[1] });
      } else {
        setCredentials(null);
        setShowSettings(true);
      }
    } catch (e) {
      setLoadError(String(e));
    }
  };

  const handleSaveSettings = async (newSettings: AppSettings, newCredentials: Credentials) => {
    setSaveError(null);
    try {
      await invoke("save_settings", { settings: newSettings });
      await invoke("save_credentials", {
        apiKey: newCredentials.api_key,
        apiSecret: newCredentials.api_secret,
      });
      setSettings(newSettings);
      setCredentials(newCredentials);
      setShowSettings(false);
    } catch (e) {
      setSaveError(String(e));
    }
  };

  // Load error — show retry screen instead of hanging on "Loading..."
  if (loadError) {
    return (
      <div className="flex flex-col items-center justify-center h-screen bg-gray-100 gap-4 p-8">
        <div className="bg-red-50 border border-red-300 text-red-700 rounded p-4 text-sm max-w-md w-full text-center">
          <p className="font-semibold mb-1">Failed to load settings</p>
          <p className="text-xs text-red-500">{loadError}</p>
        </div>
        <button
          onClick={loadAll}
          className="bg-blue-600 hover:bg-blue-700 text-white px-6 py-2 rounded transition-colors"
        >
          Retry
        </button>
      </div>
    );
  }

  // undefined = still loading; null = loaded but no credentials (Settings will show)
  if (!settings || credentials === undefined) {
    return (
      <div className="flex items-center justify-center h-screen bg-gray-100">
        <div className="text-gray-600">Loading...</div>
      </div>
    );
  }

  return (
    <div className="h-screen bg-gray-100 flex flex-col">
      <header className="bg-blue-600 text-white p-4 flex justify-between items-center">
        <h1 className="text-xl font-bold">VPN Toggle</h1>
        <button
          onClick={() => { setShowSettings(!showSettings); setSaveError(null); }}
          className="bg-blue-700 hover:bg-blue-800 px-4 py-2 rounded transition-colors"
        >
          {showSettings ? "Close Settings" : "Settings"}
        </button>
      </header>

      {/* Inline save error banner — replaces alert() */}
      {saveError && (
        <div className="bg-red-50 border-b border-red-300 text-red-700 px-4 py-2 text-sm flex justify-between items-center">
          <span>Failed to save settings: {saveError}</span>
          <button onClick={() => setSaveError(null)} className="ml-4 text-red-500 hover:text-red-700 font-bold">×</button>
        </div>
      )}

      <main className="flex-1 overflow-auto">
        {showSettings ? (
          <Settings
            settings={settings}
            credentials={credentials ?? { api_key: "", api_secret: "" }}
            onSave={handleSaveSettings}
            onCancel={() => setShowSettings(false)}
            onClearCredentials={() => {
              setCredentials(null);
              setShowSettings(true);
            }}
          />
        ) : (
          <VpnList settings={settings} />
        )}
      </main>
    </div>
  );
}

export default App;
