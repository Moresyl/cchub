import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertCircle, AlertTriangle, Info, RefreshCw, Shield, ShieldCheck } from "lucide-react";
import { t, tReplace } from "../lib/i18n";
import SecurityAuditCard, { type SecurityAuditCardResult } from "../components/SecurityAuditCard";

interface SecurityFinding {
  category: string;
  severity: string;
  title: string;
  description: string;
}

interface SecurityAuditResult {
  server_id: string;
  server_name: string;
  risk_level: string;
  findings: SecurityFinding[];
  scanned_at: string;
}

export default function Security() {
  const [results, setResults] = useState<SecurityAuditCardResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [scanned, setScanned] = useState(false);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const i = t();
  const locale = localStorage.getItem("cchub-locale") || "zh";
  const uiText = (zhText: string, enText: string, jaText?: string) => (
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText
  );

  const runAudit = useCallback(async () => {
    setLoading(true);
    try {
      const r = await invoke<SecurityAuditResult[]>("run_security_audit");
      setResults(r);
      setScanned(true);
      // Auto-expand servers with findings
      const exp: Record<string, boolean> = {};
      for (const res of r) {
        if (res.findings.length > 0) exp[res.server_id] = true;
      }
      setExpanded(exp);
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  }, []);

  useEffect(() => { void runAudit(); }, [runAudit]);

  const highCount = results.filter((r) => r.risk_level === "high").length;
  const mediumCount = results.filter((r) => r.risk_level === "medium").length;
  const lowCount = results.filter((r) => r.risk_level === "low").length;
  const totalFindings = results.reduce((sum, r) => sum + r.findings.length, 0);

  const toggleExpand = useCallback((id: string) => {
    setExpanded((prev) => ({ ...prev, [id]: !prev[id] }));
  }, []);

  const getSeverityIcon = useCallback((severity: string) => {
    switch (severity) {
      case "critical": return <AlertTriangle size={14} style={{ color: "var(--danger)" }} />;
      case "warning": return <AlertCircle size={14} style={{ color: "var(--warning)" }} />;
      default: return <Info size={14} style={{ color: "var(--text-muted)" }} />;
    }
  }, []);

  const getRiskBadge = useCallback((risk: string) => {
    switch (risk) {
      case "high": return <span className="badge badge-danger">{i.security.riskHigh}</span>;
      case "medium": return <span className="badge badge-warning">{i.security.riskMedium}</span>;
      default: return <span className="badge badge-success">{i.security.riskLow}</span>;
    }
  }, [i.security.riskHigh, i.security.riskLow, i.security.riskMedium]);

  const getCategoryLabel = useCallback((cat: string): string => {
    const labels: Record<string, string> = {
      env_secrets: uiText("敏感环境变量", "Env Secrets", "機密環境変数"),
      shell_exec: uiText("Shell 执行", "Shell Execution", "Shell 実行"),
      npx_risk: uiText("npx 自动安装", "npx Auto-install", "npx 自動インストール"),
      network_access: uiText("网络访问", "Network Access", "ネットワークアクセス"),
      file_access: uiText("文件访问", "File Access", "ファイルアクセス"),
      config_changed: uiText("配置变更", "Config Changed", "設定変更"),
    };
    return labels[cat] || cat;
  }, [locale]);

  if (loading && !scanned) {
    return (
      <div className="loading-center">
        <div className="spinner" />
        <span style={{ fontSize: 13, color: "var(--text-muted)" }}>{i.security.scanning}</span>
      </div>
    );
  }

  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{i.security.title}</h2>
          <p className="page-subtitle">
            {tReplace(i.security.serverCount, { count: results.length })}
            {totalFindings > 0 && ` · ${totalFindings} ${uiText("个发现", "findings", "件の検出")}`}
          </p>
        </div>
        <button className="btn btn-secondary btn-sm" onClick={() => { void runAudit(); }} disabled={loading}>
          <RefreshCw size={14} />{loading ? i.security.scanning : i.security.runAudit}
        </button>
      </div>

      {/* Summary Bar */}
      {scanned && (
        <div style={{ display: "flex", gap: 12, marginBottom: 24 }}>
          <div className="stat-card" style={{ flex: 1, padding: "16px 20px" }}>
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <AlertTriangle size={18} style={{ color: "var(--danger)" }} />
              <div>
                <div style={{ fontSize: 20, fontWeight: 700, color: "var(--danger)" }}>{highCount}</div>
                <div style={{ fontSize: 11, color: "var(--text-muted)" }}>{i.security.riskHigh}</div>
              </div>
            </div>
          </div>
          <div className="stat-card" style={{ flex: 1, padding: "16px 20px" }}>
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <AlertCircle size={18} style={{ color: "var(--warning)" }} />
              <div>
                <div style={{ fontSize: 20, fontWeight: 700, color: "var(--warning)" }}>{mediumCount}</div>
                <div style={{ fontSize: 11, color: "var(--text-muted)" }}>{i.security.riskMedium}</div>
              </div>
            </div>
          </div>
          <div className="stat-card" style={{ flex: 1, padding: "16px 20px" }}>
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <ShieldCheck size={18} style={{ color: "var(--success)" }} />
              <div>
                <div style={{ fontSize: 20, fontWeight: 700, color: "var(--success)" }}>{lowCount}</div>
                <div style={{ fontSize: 11, color: "var(--text-muted)" }}>{i.security.riskLow}</div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Results */}
      <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
        {!scanned || results.length === 0 ? (
          <div className="card empty-state" style={{ flex: 1 }}>
            <div className="empty-icon"><Shield size={28} style={{ color: "var(--text-muted)" }} /></div>
            <p style={{ fontSize: 15, fontWeight: 600, color: "var(--text-secondary)" }}>{i.security.noIssues}</p>
            <p style={{ fontSize: 13, color: "var(--text-muted)", marginTop: 8, maxWidth: 320 }}>{i.security.noIssuesTip}</p>
          </div>
        ) : (
          <div className="stagger" style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {results.map((result) => (
              <SecurityAuditCard
                key={result.server_id}
                result={result}
                expanded={!!expanded[result.server_id]}
                findingsCountLabel={`${result.findings.length} ${uiText("项", "findings", "件")}`}
                noIssuesLabel={uiText("未发现安全问题", "No security issues found", "セキュリティ問題は見つかりませんでした")}
                riskBadge={getRiskBadge(result.risk_level)}
                onToggle={toggleExpand}
                getFindingIcon={getSeverityIcon}
                getCategoryLabel={getCategoryLabel}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
