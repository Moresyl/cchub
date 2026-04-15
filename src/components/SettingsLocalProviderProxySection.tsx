import { memo } from "react";
import { Link2 } from "lucide-react";
import { MANAGED_APPS, type LocalProviderProxySettings, type LocalProviderProxyStatus, type ManagedAppId } from "../lib/appPreferences";
import type { Locale } from "../lib/i18n";
import SettingsManagedAppToggle from "./SettingsManagedAppToggle";
import SettingsProxyEndpointCard from "./SettingsProxyEndpointCard";

interface SettingsLocalProviderProxySectionProps {
  locale: Locale;
  settings: LocalProviderProxySettings;
  status: LocalProviderProxyStatus | null;
  saving: boolean;
  onPortChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
  onSave: () => void | Promise<void>;
  onToggleApp: (appId: ManagedAppId) => void | Promise<void>;
  onCopyEndpoint: (value: string, label: string) => void | Promise<void>;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function SettingsLocalProviderProxySectionComponent({
  locale,
  settings,
  status,
  saving,
  onPortChange,
  onSave,
  onToggleApp,
  onCopyEndpoint,
}: SettingsLocalProviderProxySectionProps) {
  const baseUrl = status?.base_url || `http://127.0.0.1:${settings.port}/proxy`;
  const enabledApps = settings.enabled_apps.filter((appId): appId is ManagedAppId => MANAGED_APPS.includes(appId as ManagedAppId));

  return (
    <div className="section-card">
      <div className="section-card-title">
        <Link2 size={17} style={{ color: "var(--text-secondary)" }} />
        {uiText(locale, "本地 Provider 代理", "Local Provider Proxy", "ローカル Provider プロキシ")}
      </div>
      <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 14 }}>
        {uiText(
          locale,
          "把 Claude / Codex / Gemini / OpenCode / OpenClaw / Hermes 的活动 Provider 改写到本机回环地址，由 CCHub 在请求时动态转发到当前选中的 Provider。切换 Provider 时无需重启代理服务。",
          "Rewrite supported app endpoints to a local loopback address so CCHub can forward requests to the currently active provider at request time. Provider switches do not require restarting the proxy service.",
          "対応 App のエンドポイントをローカルループバックへ書き換え、CCHub がリクエスト時点のアクティブ Provider へ動的転送します。Provider 切替でプロキシ再起動は不要です。",
        )}
      </p>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 12, marginBottom: 14 }}>
        <div style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
            {uiText(locale, "监听地址", "Listen Address", "待受アドレス")}
          </div>
          <div style={{ fontSize: 13, fontWeight: 600, fontFamily: "'JetBrains Mono', monospace" }}>
            {baseUrl}
          </div>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>
            {status?.running
              ? uiText(locale, "代理服务运行中", "Proxy server is running", "プロキシサーバーは稼働中です")
              : uiText(locale, "当前未运行，至少启用一个 App 后保存即可启动", "Not running. Enable at least one app and save to start it.", "現在は停止中です。少なくとも 1 つの App を有効化して保存すると起動します。")}
          </div>
        </div>

        <div style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
            {uiText(locale, "监听端口", "Listen Port", "待受ポート")}
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <input
              className="input"
              type="number"
              min={1024}
              max={65535}
              value={settings.port}
              onChange={onPortChange}
              style={{ maxWidth: 130 }}
            />
            <button
              className="btn btn-primary btn-sm"
              onClick={onSave}
              disabled={saving}
            >
              {saving
                ? uiText(locale, "保存中...", "Saving...", "保存中...")
                : uiText(locale, "保存代理设置", "Save Proxy Settings", "プロキシ設定を保存")}
            </button>
          </div>
        </div>
      </div>

      <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 14 }}>
        {MANAGED_APPS.map((appId) => (
          <SettingsManagedAppToggle
            key={`local-proxy:${appId}`}
            appId={appId}
            active={settings.enabled_apps.includes(appId)}
            disabled={saving}
            onToggle={onToggleApp}
          />
        ))}
      </div>

      <p style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 14 }}>
        {uiText(
          locale,
          "当前版本已具备本地代理接管、按 App 开关、热切换，以及基于 endpointCandidates 的候选端点自动重试。基础请求统计已接入 Dashboard；格式转换、完整故障转移队列和成本计费仍会继续补齐。",
          "This revision now includes local proxy takeover, per-app toggles, hot switching, and automatic retries across configured endpoint candidates. Basic request statistics are available on the Dashboard; format conversion, full failover queues, and cost accounting are still pending.",
          "この版ではローカルプロキシ接管、App 単位の切替、ホットスイッチ、endpointCandidates に基づく候補エンドポイント自動再試行まで利用できます。基本的なリクエスト統計は Dashboard に表示され、形式変換・完全なフェイルオーバーキュー・コスト計算は引き続き実装予定です。",
        )}
      </p>

      {enabledApps.length > 0 && (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: 10 }}>
          {enabledApps.map((appId) => (
            <SettingsProxyEndpointCard
              key={`local-proxy-endpoint:${appId}`}
              appId={appId}
              endpoint={`${baseUrl}/${appId}`}
              locale={locale}
              onCopy={onCopyEndpoint}
            />
          ))}
        </div>
      )}
    </div>
  );
}

export default memo(SettingsLocalProviderProxySectionComponent);
