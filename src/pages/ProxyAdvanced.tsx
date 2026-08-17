import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowRight, Loader2, Plus, Save, Trash2 } from "lucide-react";
import { t } from "../lib/i18n";
import { showToast } from "../components/Toast";
import LoadingState from "../components/states/LoadingState";
import CircuitBreakerPanel from "../components/CircuitBreakerPanel";
import FailoverQueueManager from "../components/FailoverQueueManager";
import { useSaveProxyAdvancedConfigMutation } from "../hooks/mutations";

interface MappingRule {
  from: string;
  to: string;
  matchMode: "contains" | "exact";
}

interface OptimizerConfig {
  enabled: boolean;
  thinkingOptimizer: boolean;
  cacheInjection: boolean;
  cacheTtl: string;
  bodyFilter: boolean;
  bodyFilterWhitelist: string[];
  modelMapper: boolean;
  modelMapperDefault: string;
  modelMapperRules: MappingRule[];
  copilotOptimizer: boolean;
  copilotMergeToolResults: boolean;
  copilotSanitizeOrphans: boolean;
  copilotStripThinking: boolean;
  copilotCompactDetection: boolean;
  copilotSubagentDetection: boolean;
  copilotModelNormalization: boolean;
  codexFieldStripping: boolean;
  circuitFailureThreshold: number;
  circuitSuccessThreshold: number;
  circuitTimeoutSecs: number;
  failoverEnabled: boolean;
  maxProfileRetries: number;
  streamingFirstByteTimeout: number;
  streamingIdleTimeout: number;
}

interface RectifierConfig {
  enabled: boolean;
  thinkingSignature: boolean;
  thinkingBudget: boolean;
}

interface ProxyAdvancedProps {
  embedded?: boolean;
  mode?: "all" | "claude" | "codex";
}

function ProxyAdvanced({ embedded = false, mode = "all" }: ProxyAdvancedProps = {}) {
  const i = t();
  const [config, setConfig] = useState<OptimizerConfig | null>(null);
  const [rectConfig, setRectConfig] = useState<RectifierConfig | null>(null);
  const [saving, setSaving] = useState(false);
  const [newWhitelist, setNewWhitelist] = useState("");
  const saveProxyAdvancedConfigMutation = useSaveProxyAdvancedConfigMutation();

  useEffect(() => {
    void loadConfig();
    void loadRectConfig();
  }, []);

  async function loadConfig() {
    try {
      const data = await invoke<OptimizerConfig>("get_optimizer_config");
      setConfig(data);
    } catch {
      setConfig({
        enabled: false,
        thinkingOptimizer: false,
        cacheInjection: false,
        cacheTtl: "5m",
        bodyFilter: false,
        bodyFilterWhitelist: [],
        modelMapper: false,
        modelMapperDefault: "",
        modelMapperRules: [],
        copilotOptimizer: false,
        copilotMergeToolResults: true,
        copilotSanitizeOrphans: true,
        copilotStripThinking: true,
        copilotCompactDetection: true,
        copilotSubagentDetection: true,
        copilotModelNormalization: true,
        codexFieldStripping: false,
        circuitFailureThreshold: 3,
        circuitSuccessThreshold: 2,
        circuitTimeoutSecs: 60,
        failoverEnabled: true,
        maxProfileRetries: 3,
        streamingFirstByteTimeout: 60,
        streamingIdleTimeout: 120,
      });
    }
  }

  async function loadRectConfig() {
    try {
      const data = await invoke<RectifierConfig>("get_rectifier_config");
      setRectConfig(data);
    } catch {
      setRectConfig({ enabled: true, thinkingSignature: true, thinkingBudget: true });
    }
  }

  async function handleSave() {
    if (!config) return;
    setSaving(true);
    try {
      await saveProxyAdvancedConfigMutation.mutateAsync({ config, rectifierConfig: rectConfig });
      showToast("success", i.proxyAdvanced.saveSuccess);
    } catch (e) {
      showToast("error", `${i.proxyAdvanced.saveFailed}: ${e}`);
    } finally {
      setSaving(false);
    }
  }

  if (!config) {
    return <LoadingState />;
  }

  function update(patch: Partial<OptimizerConfig>) {
    setConfig((prev) => (prev ? { ...prev, ...patch } : prev));
  }

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      {!embedded && (
        <div className="page-header">
          <div>
            <h2 className="page-title">{i.proxyAdvanced.title}</h2>
            <p className="page-subtitle">{i.proxyAdvanced.subtitle}</p>
          </div>
        </div>
      )}

      <div
        style={{
          flex: 1,
          overflow: "auto",
          padding: embedded ? 0 : "16px 20px",
          display: "flex",
          flexDirection: "column",
          gap: 20,
        }}
      >
        {/* Rectifier */}
        {mode !== "codex" && rectConfig && (
          <div>
            <div
              style={{
                fontSize: 12,
                fontWeight: 700,
                color: "var(--text-muted)",
                textTransform: "uppercase",
                letterSpacing: 1,
                marginBottom: 10,
              }}
            >
              {i.proxyAdvanced.rectifierTitle}
            </div>
            <ToggleRow
              label={i.proxyAdvanced.rectifierSwitch}
              description={i.proxyAdvanced.rectifierSwitchDesc}
              checked={rectConfig.enabled}
              onChange={(v) => setRectConfig((p) => (p ? { ...p, enabled: v } : p))}
            />
            <div
              style={{
                marginTop: 8,
                paddingLeft: 28,
                display: "flex",
                flexDirection: "column",
                gap: 10,
                opacity: rectConfig.enabled ? 1 : 0.5,
                pointerEvents: rectConfig.enabled ? "auto" : "none",
              }}
            >
              <ToggleRow
                label={i.proxyAdvanced.rectifierSignature}
                description={i.proxyAdvanced.rectifierSignatureDesc}
                checked={rectConfig.thinkingSignature}
                onChange={(v) => setRectConfig((p) => (p ? { ...p, thinkingSignature: v } : p))}
              />
              <ToggleRow
                label={i.proxyAdvanced.rectifierBudget}
                description={i.proxyAdvanced.rectifierBudgetDesc}
                checked={rectConfig.thinkingBudget}
                onChange={(v) => setRectConfig((p) => (p ? { ...p, thinkingBudget: v } : p))}
              />
            </div>
          </div>
        )}

        {mode !== "codex" && (
          <div style={{ borderTop: "1px solid var(--border-default)", paddingTop: 16 }}>
            <div
              style={{
                fontSize: 12,
                fontWeight: 700,
                color: "var(--text-muted)",
                textTransform: "uppercase",
                letterSpacing: 1,
                marginBottom: 10,
              }}
            >
              {i.proxyAdvanced.optimizerTitle}
            </div>
          </div>
        )}

        {/* Master switch */}
        {mode !== "codex" && (
          <ToggleRow
            label={i.proxyAdvanced.masterSwitch}
            description={i.proxyAdvanced.masterSwitchDesc}
            checked={config.enabled}
            onChange={(v) => update({ enabled: v })}
          />
        )}

        {mode !== "codex" && (
          <div
            style={{
              borderTop: "1px solid var(--border-default)",
              paddingTop: 16,
              opacity: config.enabled ? 1 : 0.5,
              pointerEvents: config.enabled ? "auto" : "none",
            }}
          >
            {/* Thinking Optimizer */}
            <ToggleRow
              label={i.proxyAdvanced.thinkingOptimizer}
              description={i.proxyAdvanced.thinkingOptimizerDesc}
              checked={config.thinkingOptimizer}
              onChange={(v) => update({ thinkingOptimizer: v })}
            />

            {/* Cache Injector */}
            <div style={{ marginTop: 16 }}>
              <ToggleRow
                label={i.proxyAdvanced.cacheInjector}
                description={i.proxyAdvanced.cacheInjectorDesc}
                checked={config.cacheInjection}
                onChange={(v) => update({ cacheInjection: v })}
              />
              {config.cacheInjection && (
                <div style={{ marginTop: 8, paddingLeft: 28 }}>
                  <label style={{ fontSize: 11, color: "var(--text-secondary)", display: "block", marginBottom: 4 }}>
                    {i.proxyAdvanced.cacheTtl}
                  </label>
                  <input
                    className="input input-sm"
                    style={{ width: 120 }}
                    value={config.cacheTtl}
                    onChange={(e) => update({ cacheTtl: e.target.value })}
                    placeholder="5m"
                  />
                </div>
              )}
            </div>

            {/* Body Filter */}
            <div style={{ marginTop: 16 }}>
              <ToggleRow
                label={i.proxyAdvanced.bodyFilter}
                description={i.proxyAdvanced.bodyFilterDesc}
                checked={config.bodyFilter}
                onChange={(v) => update({ bodyFilter: v })}
              />
              {config.bodyFilter && (
                <div style={{ marginTop: 8, paddingLeft: 28 }}>
                  <label style={{ fontSize: 11, color: "var(--text-secondary)", display: "block", marginBottom: 4 }}>
                    {i.proxyAdvanced.whitelist}
                  </label>
                  <div style={{ display: "flex", flexWrap: "wrap", gap: 4, marginBottom: 6 }}>
                    {config.bodyFilterWhitelist.map((item, idx) => (
                      <span
                        key={idx}
                        style={{
                          display: "inline-flex",
                          alignItems: "center",
                          gap: 4,
                          padding: "2px 8px",
                          borderRadius: 12,
                          background: "var(--bg-app)",
                          border: "1px solid var(--border-default)",
                          fontSize: 11,
                        }}
                      >
                        {item}
                        <button
                          style={{
                            border: "none",
                            background: "none",
                            cursor: "pointer",
                            padding: 0,
                            color: "var(--text-muted)",
                            fontSize: 11,
                          }}
                          onClick={() =>
                            update({ bodyFilterWhitelist: config.bodyFilterWhitelist.filter((_, i) => i !== idx) })
                          }
                        >
                          ×
                        </button>
                      </span>
                    ))}
                  </div>
                  <div style={{ display: "flex", gap: 6 }}>
                    <input
                      className="input input-sm"
                      style={{ width: 180 }}
                      placeholder="_fieldName"
                      value={newWhitelist}
                      onChange={(e) => setNewWhitelist(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" && newWhitelist.trim()) {
                          update({ bodyFilterWhitelist: [...config.bodyFilterWhitelist, newWhitelist.trim()] });
                          setNewWhitelist("");
                        }
                      }}
                    />
                    <button
                      className="btn btn-secondary btn-sm"
                      onClick={() => {
                        if (newWhitelist.trim()) {
                          update({ bodyFilterWhitelist: [...config.bodyFilterWhitelist, newWhitelist.trim()] });
                          setNewWhitelist("");
                        }
                      }}
                    >
                      +
                    </button>
                  </div>
                </div>
              )}
            </div>

            {/* Model Mapper */}
            <div style={{ marginTop: 16 }}>
              <ToggleRow
                label={i.proxyAdvanced.modelMapper}
                description={i.proxyAdvanced.modelMapperDesc}
                checked={config.modelMapper}
                onChange={(v) => update({ modelMapper: v })}
              />
              {config.modelMapper && (
                <div style={{ marginTop: 8, paddingLeft: 28, display: "flex", flexDirection: "column", gap: 10 }}>
                  {/* Default model */}
                  <div>
                    <label style={{ fontSize: 11, color: "var(--text-secondary)", display: "block", marginBottom: 4 }}>
                      {i.proxyAdvanced.defaultModel}
                    </label>
                    <input
                      className="input input-sm"
                      style={{ width: 280 }}
                      placeholder="claude-sonnet-4-20250514"
                      value={config.modelMapperDefault}
                      onChange={(e) => update({ modelMapperDefault: e.target.value })}
                    />
                  </div>

                  {/* Rules */}
                  <div>
                    <label style={{ fontSize: 11, color: "var(--text-secondary)", display: "block", marginBottom: 6 }}>
                      {i.proxyAdvanced.mappingRules}
                    </label>
                    {config.modelMapperRules.map((rule, idx) => (
                      <div key={idx} style={{ display: "flex", gap: 6, alignItems: "center", marginBottom: 6 }}>
                        <input
                          className="input input-sm"
                          style={{ width: 160 }}
                          placeholder={i.proxyAdvanced.fromModel}
                          value={rule.from}
                          onChange={(e) => {
                            const rules = [...config.modelMapperRules];
                            rules[idx] = { ...rule, from: e.target.value };
                            update({ modelMapperRules: rules });
                          }}
                        />
                        <ArrowRight size={12} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
                        <input
                          className="input input-sm"
                          style={{ width: 160 }}
                          placeholder={i.proxyAdvanced.toModel}
                          value={rule.to}
                          onChange={(e) => {
                            const rules = [...config.modelMapperRules];
                            rules[idx] = { ...rule, to: e.target.value };
                            update({ modelMapperRules: rules });
                          }}
                        />
                        <select
                          className="input input-sm"
                          style={{ width: 100 }}
                          value={rule.matchMode}
                          onChange={(e) => {
                            const rules = [...config.modelMapperRules];
                            rules[idx] = { ...rule, matchMode: e.target.value as "contains" | "exact" };
                            update({ modelMapperRules: rules });
                          }}
                        >
                          <option value="contains">Contains</option>
                          <option value="exact">Exact</option>
                        </select>
                        <button
                          className="btn btn-ghost btn-icon-sm"
                          onClick={() => {
                            update({ modelMapperRules: config.modelMapperRules.filter((_, i) => i !== idx) });
                          }}
                        >
                          <Trash2 size={12} />
                        </button>
                      </div>
                    ))}
                    <button
                      className="btn btn-secondary btn-sm"
                      style={{ display: "flex", alignItems: "center", gap: 5 }}
                      onClick={() =>
                        update({
                          modelMapperRules: [...config.modelMapperRules, { from: "", to: "", matchMode: "contains" }],
                        })
                      }
                    >
                      <Plus size={12} />
                      {i.proxyAdvanced.addRule}
                    </button>
                  </div>
                </div>
              )}
            </div>

            {/* Copilot Optimizer */}
            <div style={{ marginTop: 16 }}>
              <ToggleRow
                label={i.proxyAdvanced.copilotOptimizer}
                description={i.proxyAdvanced.copilotOptimizerDesc}
                checked={config.copilotOptimizer}
                onChange={(v) => update({ copilotOptimizer: v })}
              />
              {config.copilotOptimizer && (
                <div style={{ marginTop: 8, paddingLeft: 28, display: "flex", flexDirection: "column", gap: 10 }}>
                  <ToggleRow
                    label={i.proxyAdvanced.copilotMerge}
                    description={i.proxyAdvanced.copilotMergeDesc}
                    checked={config.copilotMergeToolResults}
                    onChange={(v) => update({ copilotMergeToolResults: v })}
                  />
                  <ToggleRow
                    label={i.proxyAdvanced.copilotSanitize}
                    description={i.proxyAdvanced.copilotSanitizeDesc}
                    checked={config.copilotSanitizeOrphans}
                    onChange={(v) => update({ copilotSanitizeOrphans: v })}
                  />
                  <ToggleRow
                    label={i.proxyAdvanced.copilotStripThinking}
                    description={i.proxyAdvanced.copilotStripThinkingDesc}
                    checked={config.copilotStripThinking}
                    onChange={(v) => update({ copilotStripThinking: v })}
                  />
                  <ToggleRow
                    label={i.proxyAdvanced.copilotCompact}
                    description={i.proxyAdvanced.copilotCompactDesc}
                    checked={config.copilotCompactDetection}
                    onChange={(v) => update({ copilotCompactDetection: v })}
                  />
                  <ToggleRow
                    label={i.proxyAdvanced.copilotSubagent}
                    description={i.proxyAdvanced.copilotSubagentDesc}
                    checked={config.copilotSubagentDetection}
                    onChange={(v) => update({ copilotSubagentDetection: v })}
                  />
                  <ToggleRow
                    label={i.proxyAdvanced.copilotModelNorm}
                    description={i.proxyAdvanced.copilotModelNormDesc}
                    checked={config.copilotModelNormalization}
                    onChange={(v) => update({ copilotModelNormalization: v })}
                  />
                </div>
              )}
            </div>
          </div>
        )}

        {/* Circuit Breaker */}
        {mode !== "codex" && (
          <div style={{ borderTop: "1px solid var(--border-default)", paddingTop: 16 }}>
            <div
              style={{
                fontSize: 12,
                fontWeight: 700,
                color: "var(--text-muted)",
                textTransform: "uppercase",
                letterSpacing: 1,
                marginBottom: 10,
              }}
            >
              {i.proxyAdvanced.circuitBreakerTitle}
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              <NumberRow
                label={i.proxyAdvanced.circuitFailure}
                description={i.proxyAdvanced.circuitFailureDesc}
                value={config.circuitFailureThreshold}
                onChange={(v) => update({ circuitFailureThreshold: v })}
                min={1}
                max={20}
              />
              <NumberRow
                label={i.proxyAdvanced.circuitSuccess}
                description={i.proxyAdvanced.circuitSuccessDesc}
                value={config.circuitSuccessThreshold}
                onChange={(v) => update({ circuitSuccessThreshold: v })}
                min={1}
                max={20}
              />
              <NumberRow
                label={i.proxyAdvanced.circuitTimeout}
                description={i.proxyAdvanced.circuitTimeoutDesc}
                value={config.circuitTimeoutSecs}
                onChange={(v) => update({ circuitTimeoutSecs: v })}
                min={5}
                max={600}
              />
            </div>
          </div>
        )}

        {mode !== "codex" && <CircuitBreakerPanel />}
        <FailoverQueueManager appType={mode === "claude" ? "claude" : mode === "codex" ? "codex" : undefined} />

        {/* Failover */}
        {mode !== "codex" && (
          <div style={{ borderTop: "1px solid var(--border-default)", paddingTop: 16 }}>
            <div
              style={{
                fontSize: 12,
                fontWeight: 700,
                color: "var(--text-muted)",
                textTransform: "uppercase",
                letterSpacing: 1,
                marginBottom: 10,
              }}
            >
              {i.proxyAdvanced.failoverTitle}
            </div>
            <ToggleRow
              label={i.proxyAdvanced.failoverSwitch}
              description={i.proxyAdvanced.failoverSwitchDesc}
              checked={config.failoverEnabled}
              onChange={(v) => update({ failoverEnabled: v })}
            />
            {config.failoverEnabled && (
              <div style={{ marginTop: 8, paddingLeft: 28 }}>
                <NumberRow
                  label={i.proxyAdvanced.maxRetries}
                  description={i.proxyAdvanced.maxRetriesDesc}
                  value={config.maxProfileRetries}
                  onChange={(v) => update({ maxProfileRetries: v })}
                  min={1}
                  max={10}
                />
              </div>
            )}
          </div>
        )}

        {/* Stream Timeout */}
        {mode !== "codex" && (
          <div style={{ borderTop: "1px solid var(--border-default)", paddingTop: 16 }}>
            <div
              style={{
                fontSize: 12,
                fontWeight: 700,
                color: "var(--text-muted)",
                textTransform: "uppercase",
                letterSpacing: 1,
                marginBottom: 10,
              }}
            >
              {i.proxyAdvanced.streamTimeoutTitle}
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              <NumberRow
                label={i.proxyAdvanced.streamFirstByte}
                description={i.proxyAdvanced.streamFirstByteDesc}
                value={config.streamingFirstByteTimeout}
                onChange={(v) => update({ streamingFirstByteTimeout: v })}
                min={0}
                max={600}
              />
              <NumberRow
                label={i.proxyAdvanced.streamIdle}
                description={i.proxyAdvanced.streamIdleDesc}
                value={config.streamingIdleTimeout}
                onChange={(v) => update({ streamingIdleTimeout: v })}
                min={0}
                max={600}
              />
            </div>
          </div>
        )}

        {/* Codex OAuth */}
        {mode !== "claude" && (
          <div
            style={{
              borderTop: mode === "codex" ? "none" : "1px solid var(--border-default)",
              paddingTop: mode === "codex" ? 0 : 16,
            }}
          >
            <div
              style={{
                fontSize: 12,
                fontWeight: 700,
                color: "var(--text-muted)",
                textTransform: "uppercase",
                letterSpacing: 1,
                marginBottom: 10,
              }}
            >
              {i.proxyAdvanced.codexTitle}
            </div>
            <ToggleRow
              label={i.proxyAdvanced.codexSwitch}
              description={i.proxyAdvanced.codexSwitchDesc}
              checked={config.codexFieldStripping}
              onChange={(v) => update({ codexFieldStripping: v })}
            />
          </div>
        )}
      </div>

      {/* Footer */}
      <div
        style={{
          display: "flex",
          justifyContent: "flex-end",
          padding: embedded ? "12px 0 0 0" : "12px 20px",
          borderTop: embedded ? "none" : "1px solid var(--border-default)",
          marginTop: embedded ? 12 : 0,
        }}
      >
        <button
          className="btn btn-primary btn-sm"
          onClick={() => void handleSave()}
          disabled={saving}
          style={{ display: "flex", alignItems: "center", gap: 6 }}
        >
          {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
          {i.proxyAdvanced.save}
        </button>
      </div>
    </div>
  );
}

function ToggleRow({
  label,
  description,
  checked,
  onChange,
}: {
  label: string;
  description: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16 }}>
      <div>
        <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>{label}</div>
        <div style={{ fontSize: 11, color: "var(--text-secondary)", marginTop: 2 }}>{description}</div>
      </div>
      <button
        onClick={() => onChange(!checked)}
        style={{
          position: "relative",
          width: 36,
          height: 20,
          borderRadius: 10,
          border: "none",
          cursor: "pointer",
          background: checked ? "var(--accent)" : "var(--border-default)",
          transition: "background 0.2s",
          flexShrink: 0,
        }}
      >
        <span
          style={{
            position: "absolute",
            top: 2,
            left: checked ? 18 : 2,
            width: 16,
            height: 16,
            borderRadius: "50%",
            background: "#fff",
            transition: "left 0.2s",
          }}
        />
      </button>
    </div>
  );
}

function NumberRow({
  label,
  description,
  value,
  onChange,
  min,
  max,
}: {
  label: string;
  description: string;
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
}) {
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16 }}>
      <div>
        <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>{label}</div>
        <div style={{ fontSize: 11, color: "var(--text-secondary)", marginTop: 2 }}>{description}</div>
      </div>
      <input
        className="input input-sm"
        type="number"
        style={{ width: 80, textAlign: "right", flexShrink: 0 }}
        value={value}
        min={min}
        max={max}
        onChange={(e) => {
          const n = parseInt(e.target.value, 10);
          if (isNaN(n)) return;
          const clamped = Math.max(min ?? -Infinity, Math.min(max ?? Infinity, n));
          onChange(clamped);
        }}
      />
    </div>
  );
}

export default ProxyAdvanced;
