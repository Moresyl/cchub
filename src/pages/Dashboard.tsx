/* eslint-disable react-hooks/exhaustive-deps */
import { memo, startTransition, useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plug, Zap, Package, Monitor, Terminal, Code, Wind, Activity, ArrowRight, Shield, Layers } from "lucide-react";
import { t, getLocale } from "../lib/i18n";
import { useNavigate } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { fetchMcpServersPageData, fetchSkillsPageData, useDetectTools, queryKeys } from "../hooks/queries";
import DashboardToolChip from "../components/DashboardToolChip";
import DashboardHermesRootOverride from "../components/DashboardHermesRootOverride";
import DashboardServerRow, { type DashboardServerRowServer } from "../components/DashboardServerRow";
import DashboardSkillRow, { type DashboardSkillRowSkill } from "../components/DashboardSkillRow";
import LoadingState from "../components/states/LoadingState";
import { scheduleIdleTask } from "../lib/idleTask";

type McpServer = DashboardServerRowServer;
type Skill = DashboardSkillRowSkill;
interface Plugin {
  id: string;
  name: string;
  description: string | null;
}
const TOOL_ICONS: Record<string, typeof Monitor> = { claude: Terminal, cursor: Code, windsurf: Wind, codex: Monitor };

export default function Dashboard() {
  const [servers, setServers] = useState<McpServer[]>([]);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [plugins, setPlugins] = useState<Plugin[]>([]);
  const [loading, setLoading] = useState(true);
  const refreshRequestIdRef = useRef(0);
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: tools = [], refetch: refetchTools } = useDetectTools();
  const i = t();
  const locale = getLocale();
  const uiText = (zhText: string, enText: string, jaText?: string) =>
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;

  useEffect(() => {
    let cancelled = false;

    const loadInitial = async () => {
      try {
        const [cachedServers, cachedSkills] = await Promise.all([
          invoke<McpServer[]>("get_mcp_servers").catch(() => []),
          invoke<Skill[]>("get_skills").catch(() => []),
          refetchTools().then((result) => result.data ?? []),
        ]);
        if (cancelled) {
          return;
        }

        startTransition(() => {
          setServers(cachedServers);
          setSkills(cachedSkills);
          setLoading(false);
        });
      } catch (error) {
        if (!cancelled) {
          console.error(error);
          setLoading(false);
        }
      }
    };

    const refreshScannedData = async () => {
      const requestId = refreshRequestIdRef.current + 1;
      refreshRequestIdRef.current = requestId;

      try {
        const [mcpPage, skillsPage, scannedPlugins] = await Promise.all([
          // 复用 App.tsx 预热的 mcpServersPage/skillsPage 缓存，避免重复 scan_mcp_servers / scan_skills
          queryClient.ensureQueryData({
            queryKey: queryKeys.mcpServersPage,
            queryFn: fetchMcpServersPageData,
          }),
          queryClient.ensureQueryData({
            queryKey: queryKeys.skillsPage,
            queryFn: fetchSkillsPageData,
          }),
          queryClient.ensureQueryData({
            queryKey: queryKeys.plugins,
            queryFn: () => invoke<Plugin[]>("get_plugins"),
          }),
        ]);
        if (cancelled || requestId !== refreshRequestIdRef.current) {
          return;
        }

        startTransition(() => {
          setServers(mcpPage.servers as McpServer[]);
          setSkills(skillsPage.skills as Skill[]);
          setPlugins(scannedPlugins);
        });
      } catch (error) {
        if (!cancelled && requestId === refreshRequestIdRef.current) {
          console.error(error);
        }
      }
    };

    void loadInitial();
    const cancelRefresh = scheduleIdleTask(() => void refreshScannedData(), {
      delay: 1_000,
      timeout: 6_000,
    });

    return () => {
      cancelled = true;
      cancelRefresh();
    };
  }, [refetchTools]);

  const openMcpServers = useCallback(() => navigate("/mcp-servers"), [navigate]);
  const openSkills = useCallback(() => navigate("/skills"), [navigate]);
  const openMcpClients = useCallback(() => navigate("/mcp-clients"), [navigate]);
  const openLogs = useCallback(() => navigate("/logs"), [navigate]);
  const openWorkspaces = useCallback(() => navigate("/workspaces"), [navigate]);
  const openSecurity = useCallback(() => navigate("/security"), [navigate]);

  if (loading) {
    return <LoadingState label={i.dashboard.scanning} />;
  }

  const installedTools = tools.filter((t) => t.installed);
  const activeServers = servers.filter((s) => s.status === "active");

  return (
    <div>
      {/* Stats Row */}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 16, marginBottom: 28 }}>
        <StatCard
          label={i.dashboard.mcpServers}
          value={servers.length}
          sub={`${activeServers.length} ${i.dashboard.active}`}
          icon={Plug}
          color="var(--text-primary)"
          onClick={openMcpServers}
        />
        <StatCard
          label={i.dashboard.skills}
          value={skills.length}
          sub={`${skills.filter((s) => s.plugin_id).length} ${uiText("来自插件", "from plugins", "プラグイン由来")}`}
          icon={Zap}
          color="var(--text-secondary)"
          onClick={openSkills}
        />
        <StatCard
          label={i.dashboard.plugins}
          value={plugins.length}
          icon={Package}
          color="var(--text-secondary)"
          onClick={openSkills}
        />
        <StatCard
          label={i.skills.detectedTools}
          value={installedTools.length}
          sub={`/ ${tools.length} ${uiText("已知", "known", "検出済み")}`}
          icon={Monitor}
          color="var(--text-primary)"
          onClick={openSkills}
        />
      </div>

      {/* Detected Tools Strip */}
      {installedTools.length > 0 && (
        <div className="section-card" style={{ marginBottom: 24, padding: 18 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 14 }}>
            <span
              style={{
                fontSize: 12,
                fontWeight: 600,
                color: "var(--text-muted)",
                textTransform: "uppercase",
                letterSpacing: "0.04em",
              }}
            >
              {i.skills.detectedTools}
            </span>
          </div>
          <div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}>
            {tools.map((tool) => {
              const Icon = TOOL_ICONS[tool.id] || Monitor;
              return (
                <DashboardToolChip
                  key={tool.id}
                  toolId={tool.id}
                  toolName={tool.name}
                  installed={tool.installed}
                  icon={Icon}
                  action={tool.id === "hermes" ? <DashboardHermesRootOverride /> : undefined}
                />
              );
            })}
          </div>
        </div>
      )}

      {/* Main Content Grid */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 20 }}>
        {/* MCP Servers */}
        <div className="section-card">
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 16 }}>
            <div className="section-card-title" style={{ marginBottom: 0 }}>
              <Plug size={16} style={{ color: "var(--text-secondary)" }} />
              {i.dashboard.recentMcp}
            </div>
            <button className="btn btn-ghost btn-xs" onClick={openMcpServers} style={{ gap: 4 }}>
              {uiText("查看全部", "View all", "すべて表示")}
              <ArrowRight size={12} />
            </button>
          </div>
          {servers.length === 0 ? (
            <EmptyHint text={i.dashboard.noMcpServers} sub={i.dashboard.addMcpTip} />
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
              {servers.slice(0, 3).map((s) => (
                <DashboardServerRow
                  key={s.id}
                  server={s}
                  officialLabel={i.mcp.officialPlugin}
                  communityLabel={i.mcp.communityPlugin}
                  localLabel={i.mcp.local}
                />
              ))}
              {servers.length > 3 && (
                <p style={{ fontSize: 12, color: "var(--text-muted)", textAlign: "center", padding: "8px 0" }}>
                  +{servers.length - 3} {uiText("更多", "more", "件")}
                </p>
              )}
            </div>
          )}
        </div>

        {/* Skills */}
        <div className="section-card">
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 16 }}>
            <div className="section-card-title" style={{ marginBottom: 0 }}>
              <Zap size={16} style={{ color: "var(--text-secondary)" }} />
              {i.dashboard.recentSkills}
            </div>
            <button className="btn btn-ghost btn-xs" onClick={openSkills} style={{ gap: 4 }}>
              {uiText("查看全部", "View all", "すべて表示")}
              <ArrowRight size={12} />
            </button>
          </div>
          {skills.length === 0 ? (
            <EmptyHint text={i.dashboard.noSkills} sub={i.dashboard.addSkillTip} />
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
              {skills.slice(0, 3).map((s) => (
                <DashboardSkillRow key={s.id} skill={s} />
              ))}
              {skills.length > 3 && (
                <p style={{ fontSize: 12, color: "var(--text-muted)", textAlign: "center", padding: "8px 0" }}>
                  +{skills.length - 3} {uiText("更多", "more", "件")}
                </p>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Quick Actions */}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 16, marginTop: 20 }}>
        <QuickAction
          icon={Monitor}
          label={uiText("客户端管理", "MCP Clients", "MCP クライアント")}
          desc={uiText("管理 AI 应用访问权限", "Manage AI app access", "AI アプリのアクセス権を管理")}
          onClick={openMcpClients}
        />
        <QuickAction
          icon={Activity}
          label={uiText("请求日志", "Request Logs", "リクエストログ")}
          desc={uiText("查看 MCP 活动记录", "View MCP activity", "MCP の活動履歴を表示")}
          onClick={openLogs}
        />
        <QuickAction
          icon={Layers}
          label={uiText("工作区", "Workspaces", "ワークスペース")}
          desc={uiText("切换与管理工作区", "Switch workspaces", "ワークスペースを切り替えて管理")}
          onClick={openWorkspaces}
        />
        <QuickAction
          icon={Shield}
          label={uiText("安全审计", "Security Audit", "セキュリティ監査")}
          desc={uiText("扫描 MCP 安全风险", "Scan MCP security risks", "MCP のセキュリティリスクをスキャン")}
          onClick={openSecurity}
        />
      </div>
    </div>
  );
}

const StatCard = memo(function StatCard({
  label,
  value,
  sub,
  icon: Icon,
  color,
  onClick,
}: {
  label: string;
  value: number;
  sub?: string;
  icon: typeof Plug;
  color: string;
  onClick?: () => void;
}) {
  return (
    <div className="stat-card" onClick={onClick} style={{ cursor: onClick ? "pointer" : undefined }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12 }}>
        <div className="icon-box" style={{ background: `${color}15`, width: 36, height: 36, borderRadius: 6 }}>
          <Icon size={17} style={{ color }} />
        </div>
        <span style={{ fontSize: 26, fontWeight: 800, color, letterSpacing: "-0.03em" }}>{value}</span>
      </div>
      <p style={{ fontSize: 13, color: "var(--text-muted)", fontWeight: 500 }}>{label}</p>
      {sub && <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2, opacity: 0.7 }}>{sub}</p>}
    </div>
  );
});

const QuickAction = memo(function QuickAction({
  icon: Icon,
  label,
  desc,
  onClick,
}: {
  icon: typeof Plug;
  label: string;
  desc: string;
  onClick: () => void;
}) {
  return (
    <div className="card card-interactive" style={{ padding: "18px 20px", cursor: "pointer" }} onClick={onClick}>
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <div className="icon-box" style={{ background: "var(--bg-elevated)", width: 36, height: 36, borderRadius: 6 }}>
          <Icon size={16} style={{ color: "var(--text-secondary)" }} />
        </div>
        <div>
          <p style={{ fontSize: 13, fontWeight: 600 }}>{label}</p>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 2 }}>{desc}</p>
        </div>
      </div>
    </div>
  );
});

const EmptyHint = memo(function EmptyHint({ text, sub }: { text: string; sub: string }) {
  return (
    <div style={{ padding: "28px 16px", textAlign: "center" }}>
      <p style={{ fontSize: 13, color: "var(--text-muted)" }}>{text}</p>
      <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4, opacity: 0.7 }}>{sub}</p>
    </div>
  );
});
