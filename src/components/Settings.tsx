import { useState } from "react";
import { AppSettings, VpnGateway, Credentials } from "../App";

interface SettingsProps {
  settings: AppSettings;
  credentials: Credentials;
  onSave: (settings: AppSettings, credentials: Credentials) => void;
  onCancel: () => void;
}

function validateSettings(data: AppSettings, creds: Credentials): string | null {
  if (!data.base_url.trim()) return "Base URL is required.";
  if (!/^https?:\/\/.+/.test(data.base_url.trim())) return "Base URL must start with http:// or https://";
  if (!creds.api_key.trim()) return "API Key is required.";
  if (!creds.api_secret.trim()) return "API Secret is required.";
  for (let i = 0; i < data.gateways.length; i++) {
    const g = data.gateways[i];
    if (!g.display_name.trim()) return `Gateway ${i + 1}: Display Name is required.`;
    if (!g.gateway_name.trim()) return `Gateway ${i + 1}: Gateway Name is required.`;
    if (!g.alias_name.trim()) return `Gateway ${i + 1}: Alias Name is required.`;
  }
  // Check for duplicate alias names
  const aliasNames = data.gateways.map((g) => g.alias_name.trim());
  const unique = new Set(aliasNames);
  if (unique.size !== aliasNames.length) return "Duplicate Alias Names detected — each gateway must have a unique alias.";
  return null;
}

function Settings({ settings, credentials, onSave, onCancel }: SettingsProps) {
  const [formData, setFormData] = useState<AppSettings>(settings);
  const [formCredentials, setFormCredentials] = useState<Credentials>(credentials);
  const [validationError, setValidationError] = useState<string | null>(null);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const err = validateSettings(formData, formCredentials);
    if (err) {
      setValidationError(err);
      return;
    }
    setValidationError(null);
    onSave(formData, formCredentials);
  };

  const addGateway = () => {
    setFormData({
      ...formData,
      gateways: [
        ...formData.gateways,
        { display_name: "", gateway_name: "", alias_name: "" },
      ],
    });
  };

  const removeGateway = (index: number) => {
    const name = formData.gateways[index].display_name || `Gateway ${index + 1}`;
    if (!window.confirm(`Remove "${name}"? This cannot be undone.`)) return;
    setFormData({
      ...formData,
      gateways: formData.gateways.filter((_, i) => i !== index),
    });
  };

  const updateGateway = (
    index: number,
    field: keyof VpnGateway,
    value: string
  ) => {
    const newGateways = [...formData.gateways];
    newGateways[index] = { ...newGateways[index], [field]: value };
    setFormData({ ...formData, gateways: newGateways });
  };

  return (
    <div className="p-6 max-w-3xl mx-auto">
      <h2 className="text-2xl font-bold mb-6 text-gray-800">Settings</h2>

      {validationError && (
        <div className="mb-4 bg-red-50 border border-red-300 text-red-700 rounded p-3 text-sm">
          {validationError}
        </div>
      )}

      <form onSubmit={handleSubmit} className="space-y-6">
        <div className="bg-white rounded-lg shadow p-6 space-y-4">
          <h3 className="text-lg font-semibold text-gray-700 mb-4">
            OPNsense Configuration
          </h3>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Base URL <span className="text-red-500">*</span>
            </label>
            <input
              type="text"
              value={formData.base_url}
              onChange={(e) =>
                setFormData({ ...formData, base_url: e.target.value })
              }
              className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="https://10.0.0.1:444"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              API Key <span className="text-red-500">*</span>
            </label>
            <input
              type="text"
              value={formCredentials.api_key}
              onChange={(e) =>
                setFormCredentials({ ...formCredentials, api_key: e.target.value })
              }
              className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="Your OPNsense API Key"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              API Secret <span className="text-red-500">*</span>
            </label>
            <input
              type="password"
              value={formCredentials.api_secret}
              onChange={(e) =>
                setFormCredentials({ ...formCredentials, api_secret: e.target.value })
              }
              className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="Your OPNsense API Secret"
            />
          </div>
        </div>

        <div className="bg-white rounded-lg shadow p-6">
          <div className="flex justify-between items-center mb-4">
            <h3 className="text-lg font-semibold text-gray-700">
              VPN Gateways
            </h3>
            <button
              type="button"
              onClick={addGateway}
              className="bg-green-500 hover:bg-green-600 text-white px-4 py-2 rounded transition-colors"
            >
              Add Gateway
            </button>
          </div>

          {formData.gateways.length === 0 ? (
            <p className="text-gray-500 text-center py-4">
              No gateways configured. Click "Add Gateway" to get started.
            </p>
          ) : (
            <div className="space-y-4">
              {formData.gateways.map((gateway, index) => {
                return (
                  <div
                    key={`gateway-${index}`}
                    className="border border-gray-200 rounded-md p-4 space-y-3"
                  >
                    <div className="flex justify-between items-center mb-2">
                      <span className="text-sm font-medium text-gray-600">
                        Gateway {index + 1}
                      </span>
                      <button
                        type="button"
                        onClick={() => removeGateway(index)}
                        className="text-red-500 hover:text-red-700 text-sm"
                      >
                        Remove
                      </button>
                    </div>

                    <div>
                      <label className="block text-sm font-medium text-gray-700 mb-1">
                        Display Name <span className="text-red-500">*</span>
                      </label>
                      <input
                        type="text"
                        value={gateway.display_name}
                        onChange={(e) =>
                          updateGateway(index, "display_name", e.target.value)
                        }
                        className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                        placeholder="e.g., US East VPN"
                      />
                    </div>

                    <div>
                      <label className="block text-sm font-medium text-gray-700 mb-1">
                        Gateway Name <span className="text-red-500">*</span>
                      </label>
                      <input
                        type="text"
                        value={gateway.gateway_name}
                        onChange={(e) =>
                          updateGateway(index, "gateway_name", e.target.value)
                        }
                        className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                        placeholder="OPNsense gateway name (e.g. WAN_VPN)"
                      />
                      <p className="text-xs text-gray-400 mt-1">Used to check if the gateway is online</p>
                    </div>

                    <div>
                      <label className="block text-sm font-medium text-gray-700 mb-1">
                        Alias Name <span className="text-red-500">*</span>
                      </label>
                      <input
                        type="text"
                        value={gateway.alias_name}
                        onChange={(e) =>
                          updateGateway(index, "alias_name", e.target.value)
                        }
                        className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                        placeholder="Firewall alias name (e.g. vpn_devices)"
                      />
                      <p className="text-xs text-gray-400 mt-1">Used to add/remove this device's IP for VPN routing</p>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        <div className="flex space-x-3">
          <button
            type="submit"
            className="flex-1 bg-blue-600 hover:bg-blue-700 text-white font-semibold py-3 rounded-lg transition-colors"
          >
            Save Settings
          </button>
          <button
            type="button"
            onClick={onCancel}
            className="flex-1 bg-gray-300 hover:bg-gray-400 text-gray-800 font-semibold py-3 rounded-lg transition-colors"
          >
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}

export default Settings;
