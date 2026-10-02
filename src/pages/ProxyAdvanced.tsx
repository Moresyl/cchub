import { useId, useState } from "react";
import { ArrowRight, Loader2, Plus, Save, Trash2 } from "lucide-react";
import { t } from "../lib/i18n";
import { showToast } from "../components/Toast";
import LoadingState from "../components/states/LoadingState";
import ErrorState from "../components/states/ErrorState";
import CircuitBreakerPanel from "../components/CircuitBreakerPanel";
import FailoverQueueManager from "../components/FailoverQueueManager";
import ProviderRoutingPanel from "../components/ProviderRoutingPanel";
import { Input } from "../components/ui/input";
import { Button } from "../components/ui/button";
import NumberRow from "./ProxyAdvanced/NumberRow";
import { SimpleSelect } from "../components/ui/simple-select";
import { Switch } from "../components/ui/switch";
import useSettings from "./ProxyAdvanced/useSettings";
import type { OptimizerConfig } from "./ProxyAdvanced/types";
import AdmissionPanel from "./ProxyAdvanced/AdmissionPanel";
import SettingsNotice from "./ProxyAdvanced/SettingsNotice";

interface ProxyAdvancedProps {
  embedded?: boolean;
  mode?: "all" | "claude" | "codex";
}

function ProxyAdvanced({ embedded = false, mode = "all" }: ProxyAdvancedProps = {}) {
  const i = t();
  const { config, setConfig, rectConfig, setRectConfig, loading, loadError, saveError, saving, load, save } =
    useSettings();
  const [newWhitelist, setNewWhitelist] = useState("");

  async function handleSave() {
    try {
      if (await save()) showToast("success", i.proxyAdvanced.saveSuccess);
    } catch {
      showToast("error", i.proxyAdvanced.saveFailed);
    }
  }

  if (!config) {
    return loadError ? (
      <ErrorState
        title={i.proxyAdvanced.readFailed}
        message={i.proxyAdvanced.readFailedDesc}
        retryLabel={i.proxyAdvanced.reload}
        onRetry={() => void load()}
      />
    ) : (
      <LoadingState />
    );
  }

  function update(patch: Partial<OptimizerConfig>) {
    setConfig((prev) => (prev ? { ...prev, ...patch } : prev));
  }

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      {!embedded && (
        <div
          className="page-header"
          style={{
            padding: "16px 20px",
            marginBottom: 0,
            flexShrink: 0,
            borderBottom: "1px solid var(--border-default)",
          }}
        >
          <div>
            <h2 className="page-title">{i.proxyAdvanced.title}</h2>
            <p className="page-subtitle">{i.proxyAdvanced.subtitle}</p>
          </div>
        </div>
      )}

      {(loadError || saveError) && (
        <SettingsNotice
          title={loadError ? i.proxyAdvanced.readFailed : i.proxyAdvanced.saveFailed}
          message={loadError ? i.proxyAdvanced.readFailedDesc : i.proxyAdvanced.saveFailedDesc}
          reloadLabel={i.proxyAdvanced.reload}
          loading={loading}
          reload={() => void load()}
        />
      )}
      <fieldset
        disabled={saving || loading || loadError || saveError}
        aria-busy={loading || saving}
        aria-label={i.proxyAdvanced.title}
        style={{
          border: 0,
          margin: 0,
          minWidth: 0,
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
                fontWeight: "var(--font-weight-semibold)",
                color: "var(--text-muted)",
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
            <fieldset
              disabled={!rectConfig.enabled}
              style={{
                border: 0,
                margin: 0,
                padding: 0,
                marginTop: 8,
                paddingLeft: 28,
                display: "flex",
                flexDirection: "column",
                gap: 10,
                opacity: rectConfig.enabled ? 1 : 0.5,
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
            </fieldset>
          </div>
        )}

        {mode !== "codex" && (
          <div style={{ borderTop: "1px solid var(--border-default)", paddingTop: 16 }}>
            <div
              style={{
                fontSize: 12,
                fontWeight: "var(--font-weight-semibold)",
                color: "var(--text-muted)",
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
          <fieldset
            disabled={!config.enabled}
            style={{
              border: 0,
              minWidth: 0,
              borderTop: "1px solid var(--border-default)",
              margin: 0,
              padding: 0,
              paddingTop: 16,
              opacity: config.enabled ? 1 : 0.5,
            }}
          >
            <legend className="sr-only">{i.proxyAdvanced.optimizerTitle}</legend>
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
                  <label style={{ fontSize: 12, color: "var(--text-secondary)", display: "block", marginBottom: 4 }}>
                    {i.proxyAdvanced.cacheTtl}
                  </label>
                  <Input
                    aria-label={i.proxyAdvanced.cacheTtl}
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
                  <label style={{ fontSize: 12, color: "var(--text-secondary)", display: "block", marginBottom: 4 }}>
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
                          borderRadius: 6,
                          background: "var(--bg-app)",
                          border: "1px solid var(--border-default)",
                          fontSize: 11,
                        }}
                      >
                        {item}
                        <Button
                          variant="ghost"
                          size="icon-xs"
                          aria-label={`${i.common.delete} ${item}`}
                          onClick={() =>
                            update({ bodyFilterWhitelist: config.bodyFilterWhitelist.filter((_, i) => i !== idx) })
                          }
                        >
                          ×
                        </Button>
                      </span>
                    ))}
                  </div>
                  <div style={{ display: "flex", gap: 6 }}>
                    <Input
                      aria-label={i.proxyAdvanced.whitelist}
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
                    <Button
                      variant="secondary"
                      size="icon"
                      aria-label={i.proxyAdvanced.addWhitelistField}
                      disabled={!newWhitelist.trim()}
                      onClick={() => {
                        if (newWhitelist.trim()) {
                          update({ bodyFilterWhitelist: [...config.bodyFilterWhitelist, newWhitelist.trim()] });
                          setNewWhitelist("");
                        }
                      }}
                    >
                      <Plus size={14} />
                    </Button>
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
                    <label style={{ fontSize: 12, color: "var(--text-secondary)", display: "block", marginBottom: 4 }}>
                      {i.proxyAdvanced.defaultModel}
                    </label>
                    <Input
                      aria-label={i.proxyAdvanced.defaultModel}
                      style={{ width: 280, maxWidth: "100%" }}
                      placeholder="claude-sonnet-4-20250514"
                      value={config.modelMapperDefault}
                      onChange={(e) => update({ modelMapperDefault: e.target.value })}
                    />
                  </div>

                  {/* Rules */}
                  <div>
                    <label style={{ fontSize: 12, color: "var(--text-secondary)", display: "block", marginBottom: 6 }}>
                      {i.proxyAdvanced.mappingRules}
                    </label>
                    {config.modelMapperRules.map((rule, idx) => (
                      <div
                        key={idx}
                        style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center", marginBottom: 8 }}
                      >
                        <Input
                          aria-label={`${i.proxyAdvanced.fromModel} ${idx + 1}`}
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
                        <Input
                          aria-label={`${i.proxyAdvanced.toModel} ${idx + 1}`}
                          style={{ width: 160 }}
                          placeholder={i.proxyAdvanced.toModel}
                          value={rule.to}
                          onChange={(e) => {
                            const rules = [...config.modelMapperRules];
                            rules[idx] = { ...rule, to: e.target.value };
                            update({ modelMapperRules: rules });
                          }}
                        />
                        <SimpleSelect
                          className="w-[100px]"
                          value={rule.matchMode}
                          ariaLabel={i.proxyAdvanced.mappingRules}
                          options={[
                            { value: "contains", label: "Contains" },
                            { value: "exact", label: "Exact" },
                          ]}
                          onValueChange={(value) => {
                            const rules = [...config.modelMapperRules];
                            rules[idx] = { ...rule, matchMode: value as "contains" | "exact" };
                            update({ modelMapperRules: rules });
                          }}
                        />
                        <Button
                          variant="ghost"
                          size="icon"
                          aria-label={`${i.common.delete} ${i.proxyAdvanced.mappingRules} ${idx + 1}`}
                          onClick={() => {
                            update({ modelMapperRules: config.modelMapperRules.filter((_, i) => i !== idx) });
                          }}
                        >
                          <Trash2 size={12} />
                        </Button>
                      </div>
                    ))}
                    <Button
                      variant="secondary"
                      onClick={() =>
                        update({
                          modelMapperRules: [...config.modelMapperRules, { from: "", to: "", matchMode: "contains" }],
                        })
                      }
                    >
                      <Plus size={12} />
                      {i.proxyAdvanced.addRule}
                    </Button>
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
          </fieldset>
        )}

        {/* Circuit Breaker */}
        {mode !== "codex" && (
          <div style={{ borderTop: "1px solid var(--border-default)", paddingTop: 16 }}>
            <div
              style={{
                fontSize: 12,
                fontWeight: "var(--font-weight-semibold)",
                color: "var(--text-muted)",
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
        <ProviderRoutingPanel
          key={mode}
          appType={mode === "claude" ? "claude" : mode === "codex" ? "codex" : undefined}
        />
        <FailoverQueueManager appType={mode === "claude" ? "claude" : mode === "codex" ? "codex" : undefined} />

        {/* Failover */}
        {mode !== "codex" && (
          <div style={{ borderTop: "1px solid var(--border-default)", paddingTop: 16 }}>
            <div
              style={{
                fontSize: 12,
                fontWeight: "var(--font-weight-semibold)",
                color: "var(--text-muted)",
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

        {/* Request Timeout */}
        <div style={{ borderTop: "1px solid var(--border-default)", paddingTop: 16 }}>
          <div
            style={{
              fontSize: 12,
              fontWeight: "var(--font-weight-semibold)",
              color: "var(--text-muted)",
              marginBottom: 10,
            }}
          >
            {i.proxyAdvanced.streamTimeoutTitle}
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <NumberRow
              label={i.proxyAdvanced.nonStreamTimeout}
              description={i.proxyAdvanced.nonStreamTimeoutDesc}
              value={config.nonStreamingTimeout}
              onChange={(v) => update({ nonStreamingTimeout: v })}
              min={0}
              max={86400}
            />
            <NumberRow
              label={i.proxyAdvanced.streamFirstByte}
              description={i.proxyAdvanced.streamFirstByteDesc}
              value={config.streamingFirstByteTimeout}
              onChange={(v) => update({ streamingFirstByteTimeout: v })}
              min={0}
              max={86400}
            />
            <NumberRow
              label={i.proxyAdvanced.streamIdle}
              description={i.proxyAdvanced.streamIdleDesc}
              value={config.streamingIdleTimeout}
              onChange={(v) => update({ streamingIdleTimeout: v })}
              min={0}
              max={86400}
            />
          </div>
        </div>

        <AdmissionPanel config={config.admission} onChange={(admission) => update({ admission })} />

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
                fontWeight: "var(--font-weight-semibold)",
                color: "var(--text-muted)",
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
      </fieldset>

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
        <Button onClick={() => void handleSave()} disabled={saving || loading || loadError || saveError}>
          {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
          {i.proxyAdvanced.save}
        </Button>
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
  const descriptionId = useId();
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16 }}>
      <div style={{ minWidth: 0 }}>
        <div style={{ fontSize: 14, fontWeight: "var(--font-weight-medium)", color: "var(--text-primary)" }}>
          {label}
        </div>
        <div id={descriptionId} style={{ fontSize: 12, color: "var(--text-secondary)", marginTop: 4 }}>
          {description}
        </div>
      </div>
      <Switch checked={checked} onCheckedChange={onChange} aria-label={label} aria-describedby={descriptionId} />
    </div>
  );
}

export default ProxyAdvanced;
