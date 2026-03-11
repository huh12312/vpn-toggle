import { useState } from "react";
import { AppSettings, VpnGateway } from "../App";

interface SettingsProps {
  settings: AppSettings;
  onSave: (settings: AppSettings) => void;
  onCancel: () => void;
}

function Settings({ settings, onSave, onCancel }: SettingsProps) {
  const [formData, setFormData] = useState<AppSettings>(settings);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSave(formData);
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

      <form onSubmit={handleSubmit} className="space-y-6">
        <div className="bg-white rounded-lg shadow p-6 space-y-4">
          <h3 className="text-lg font-semibold text-gray-700 mb-4">
            OPNsense Configuration
          </h3>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Base URL
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
              API Key
            </label>
            <input
              type="text"
              value={formData.api_key}
              onChange={(e) =>
                setFormData({ ...formData, api_key: e.target.value })
              }
              className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="Your OPNsense API Key"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              API Secret
            </label>
            <input
              type="password"
              value={formData.api_secret}
              onChange={(e) =>
                setFormData({ ...formData, api_secret: e.target.value })
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
              {formData.gateways.map((gateway, index) => (
                <div
                  key={index}
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
                      Display Name
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
                      Gateway Name
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
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">
                      Alias Name
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
                  </div>
                </div>
              ))}
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
