import { memo, useState, type ChangeEventHandler, type KeyboardEventHandler } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Loader2, Radar, Wifi } from "lucide-react";
import { showToast } from "./Toast";

interface SettingsNetworkProxySectionProps {
  title: string;
  description: string;
  hint: string;
  proxyUrl: string;
  proxySaved: boolean;
  saveLabel: string;
  testLabel: string;
  scanLabel: string;
  detectedLabel: string;
  placeholder: string;
  onProxyChange: ChangeEventHandler<HTMLInputElement>;
  onProxyKeyDown: KeyboardEventHandler<HTMLInputElement>;
  onSave: () => void | Promise<void>;
}

function SettingsNetworkProxySectionComponent({
  title,
  description,
  hint,
  proxyUrl,
  proxySaved,
  saveLabel,
  testLabel,
  scanLabel,
  detectedLabel,
  placeholder,
  onProxyChange,
  onProxyKeyDown,
  onSave,
}: SettingsNetworkProxySectionProps) {
  const [testing, setTesting] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [detected, setDetected] = useState<Array<{ url: string; proxyType: string; port: number }>>([]);

  const testProxy = async () => {
    if (!proxyUrl.trim()) return;
    setTesting(true);
    try {
      const result = await invoke<{ success: boolean; latencyMs: number; error?: string }>("test_proxy_url", {
        proxyUrl,
      });
      showToast(
        result.success ? "success" : "error",
        result.success ? `Proxy reachable · ${result.latencyMs}ms` : result.error || "Proxy test failed",
      );
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setTesting(false);
    }
  };

  const scanProxies = async () => {
    setScanning(true);
    try {
      const result = await invoke<Array<{ url: string; proxyType: string; port: number }>>("scan_local_proxies");
      setDetected(result);
      showToast(
        "success",
        result.length ? `Detected ${result.length} local proxy endpoints` : "No local proxy detected",
      );
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setScanning(false);
    }
  };

  return (
    <div className="section-card">
      <div className="section-card-title">
        <Wifi size={17} style={{ color: "var(--text-secondary)" }} />
        {title}
      </div>
      <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 12 }}>{description}</p>
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
        <input
          className="input"
          style={{ flex: 1, fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}
          placeholder={placeholder}
          value={proxyUrl}
          onChange={onProxyChange}
          onKeyDown={onProxyKeyDown}
        />
        <button className="btn btn-primary btn-sm" style={{ gap: 5 }} onClick={onSave}>
          {proxySaved ? <Check size={13} style={{ color: "var(--success)" }} /> : <Check size={13} />}
          {saveLabel}
        </button>
        <button
          className="btn btn-secondary btn-sm"
          type="button"
          disabled={!proxyUrl.trim() || testing}
          onClick={() => void testProxy()}
        >
          {testing ? <Loader2 size={13} className="animate-spin" /> : <Wifi size={13} />}
          {testLabel}
        </button>
        <button
          className="btn btn-secondary btn-sm"
          type="button"
          disabled={scanning}
          onClick={() => void scanProxies()}
        >
          {scanning ? <Loader2 size={13} className="animate-spin" /> : <Radar size={13} />}
          {scanLabel}
        </button>
      </div>
      {detected.length > 0 && (
        <div style={{ marginTop: 8, fontSize: 11, color: "var(--text-muted)" }}>
          {detectedLabel}: {detected.map((item) => item.url).join(", ")}
        </div>
      )}
      <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>{hint}</p>
    </div>
  );
}

export default memo(SettingsNetworkProxySectionComponent);
