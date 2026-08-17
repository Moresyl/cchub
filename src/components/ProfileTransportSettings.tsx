import { useMemo } from "react";
import { Plus, Trash2 } from "lucide-react";

interface ProfileTransportSettingsProps {
  localeText: (zh: string, en: string, ja?: string) => string;
  customUserAgent: string;
  requestHeaders: Record<string, string>;
  requestHeaderOverrides: string;
  requestBodyOverrides: string;
  onChange: (next: {
    customUserAgent?: string;
    requestHeaders?: Record<string, string>;
    requestHeaderOverrides?: string;
    requestBodyOverrides?: string;
  }) => void;
}

const DRAFT_HEADER_PREFIX = "draft-header:";
const HEADER_NAME_PATTERN = /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/;

function nextDraftHeader(headers: Record<string, string>): string {
  let suffix = Date.now();
  while (`${DRAFT_HEADER_PREFIX}${suffix}` in headers) suffix += 1;
  return `${DRAFT_HEADER_PREFIX}${suffix}`;
}

export default function ProfileTransportSettings({
  localeText,
  customUserAgent,
  requestHeaders,
  requestHeaderOverrides,
  requestBodyOverrides,
  onChange,
}: ProfileTransportSettingsProps) {
  const entries = useMemo(() => Object.entries(requestHeaders), [requestHeaders]);
  const updateHeaders = (next: Record<string, string>) => onChange({ requestHeaders: next });
  const parseOverrideError = (value: string, body: boolean) => {
    if (!value.trim()) return "";
    try {
      const parsed = JSON.parse(value) as unknown;
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return "JSON must be an object";
      if (body && Object.prototype.hasOwnProperty.call(parsed, "stream"))
        return 'The body override cannot include "stream"';
      return "";
    } catch (error) {
      return error instanceof Error ? error.message : "Invalid JSON";
    }
  };
  const headerOverrideError = parseOverrideError(requestHeaderOverrides, false);
  const bodyOverrideError = parseOverrideError(requestBodyOverrides, true);

  const addHeader = () => {
    if (entries.length >= 64) return;
    updateHeaders({ ...requestHeaders, [nextDraftHeader(requestHeaders)]: "" });
  };

  const removeHeader = (name: string) => {
    const next = { ...requestHeaders };
    delete next[name];
    updateHeaders(next);
  };

  const renameHeader = (oldName: string, rawName: string) => {
    const name = rawName.trim();
    if (!name || !HEADER_NAME_PATTERN.test(name) || name.length > 128) return;
    if (Object.keys(requestHeaders).some((key) => key !== oldName && key.toLowerCase() === name.toLowerCase())) return;
    const next: Record<string, string> = {};
    for (const [key, value] of entries) next[key === oldName ? name : key] = value;
    updateHeaders(next);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <h3 className="section-card-title" style={{ margin: 0 }}>
        {localeText("请求传输覆盖", "Request Transport", "リクエスト転送")}
      </h3>
      <p style={{ margin: 0, fontSize: 12, color: "var(--text-muted)", lineHeight: 1.6 }}>
        {localeText(
          "仅在本地代理接管后生效。认证、Host、内容长度等由 CCHub 管理的 Header 不允许覆盖。",
          "Applied when the local proxy is active. Auth, Host, content length, and other managed headers cannot be overridden.",
          "ローカルプロキシ有効時のみ適用されます。認証、Host、長さなどの管理対象 Header は上書きできません。",
        )}
      </p>
      <label className="field-label" htmlFor="profile-custom-user-agent">
        {localeText("自定义 User-Agent", "Custom User-Agent", "カスタム User-Agent")}
      </label>
      <input
        id="profile-custom-user-agent"
        className="input"
        value={customUserAgent}
        maxLength={512}
        onChange={(event) => onChange({ customUserAgent: event.target.value })}
        placeholder="CCHub/1.0"
        autoComplete="off"
      />
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
        <span className="field-label">
          {localeText("附加请求 Header", "Additional Request Headers", "追加リクエスト Header")}
        </span>
        <button
          className="btn btn-secondary btn-sm"
          type="button"
          onClick={addHeader}
          disabled={entries.length >= 64}
          style={{ gap: 6 }}
        >
          <Plus size={13} />
          {localeText("添加", "Add", "追加")}
        </button>
      </div>
      {entries.length === 0 ? (
        <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
          {localeText("未配置附加 Header", "No additional headers configured", "追加 Header は未設定です")}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {entries.map(([name, value]) => {
            const isDraft = name.startsWith(DRAFT_HEADER_PREFIX);
            return (
              <div
                key={name}
                style={{ display: "grid", gridTemplateColumns: "minmax(0, 1fr) minmax(0, 1fr) auto", gap: 8 }}
              >
                <input
                  className="input"
                  value={isDraft ? "" : name}
                  placeholder="X-Provider-Tag"
                  onChange={(event) => renameHeader(name, event.target.value)}
                  aria-label={localeText("Header 名称", "Header name", "Header 名")}
                />
                <input
                  className="input"
                  value={value}
                  maxLength={4096}
                  placeholder="cchub"
                  onChange={(event) => updateHeaders({ ...requestHeaders, [name]: event.target.value })}
                  aria-label={localeText("Header 值", "Header value", "Header 値")}
                />
                <button
                  className="btn btn-ghost btn-icon-sm"
                  type="button"
                  onClick={() => removeHeader(name)}
                  aria-label={localeText("删除 Header", "Remove header", "Header を削除")}
                >
                  <Trash2 size={14} />
                </button>
              </div>
            );
          })}
        </div>
      )}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))", gap: 12 }}>
        <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <span className="field-label">
            {localeText("代理 Header 覆盖", "Proxy Header Overrides", "プロキシ Header 上書き")}
          </span>
          <textarea
            className="input"
            value={requestHeaderOverrides}
            onChange={(event) => onChange({ requestHeaderOverrides: event.target.value })}
            placeholder={'{\n  "X-Provider-Tag": "cchub"\n}'}
            style={{
              minHeight: 110,
              resize: "vertical",
              fontFamily: "ui-monospace, SFMono-Regular, monospace",
              fontSize: 12,
            }}
            aria-invalid={Boolean(headerOverrideError)}
          />
          {headerOverrideError && <span style={{ color: "var(--danger)", fontSize: 11 }}>{headerOverrideError}</span>}
        </label>
        <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <span className="field-label">
            {localeText("代理 Body 覆盖", "Proxy Body Overrides", "プロキシ Body 上書き")}
          </span>
          <textarea
            className="input"
            value={requestBodyOverrides}
            onChange={(event) => onChange({ requestBodyOverrides: event.target.value })}
            placeholder={'{\n  "temperature": 0.2\n}'}
            style={{
              minHeight: 110,
              resize: "vertical",
              fontFamily: "ui-monospace, SFMono-Regular, monospace",
              fontSize: 12,
            }}
            aria-invalid={Boolean(bodyOverrideError)}
          />
          {bodyOverrideError && <span style={{ color: "var(--danger)", fontSize: 11 }}>{bodyOverrideError}</span>}
        </label>
      </div>
    </div>
  );
}
