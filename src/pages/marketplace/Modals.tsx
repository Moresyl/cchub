/* eslint-disable @typescript-eslint/no-explicit-any */
import { open as shellOpen } from "@tauri-apps/plugin-shell";
import { CheckCircle, Download, Edit3, ExternalLink, Globe, Key, Plug, Plus, Trash2, X, Zap } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import {
  useCallback,
  useEffect,
  useState,
  type ChangeEvent,
  type Dispatch,
  type KeyboardEvent,
  type SetStateAction,
} from "react";

import ConfirmDialog from "../../components/ConfirmDialog";
import MarketplaceCustomSourceRow from "../../components/MarketplaceCustomSourceRow";
import MarketplaceRecommendedRepoRow from "../../components/MarketplaceRecommendedRepoRow";
import MarkdownPreview from "../../components/MarkdownPreview";
import { t } from "../../lib/i18n";
import { MCP_CATEGORY_ZH, type RegistryEntry, type SkillEntry } from "./helpers";

type ToastFn = (zh: string, en: string) => string;

export interface EnvModalProps {
  locale: string;
  showEnvModal: RegistryEntry | null;
  envValues: Record<string, string>;
  setShowEnvModal: (v: RegistryEntry | null) => void;
  setEnvValues: (updater: (prev: Record<string, string>) => Record<string, string>) => void;
  doInstallMcp: (entry: RegistryEntry, env: Record<string, string>) => void;
}

export function EnvModal(props: EnvModalProps) {
  const { locale, showEnvModal, envValues } = props;
  const i = t();
  if (!showEnvModal) return null;
  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "var(--bg-overlay)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
      onClick={() => props.setShowEnvModal(null)}
    >
      <div
        className="section-card"
        style={{ width: 440, maxHeight: "80vh", overflowY: "auto" }}
        onClick={(e) => e.stopPropagation()}
      >
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 20 }}>
          <h3 style={{ fontSize: 16, fontWeight: 700 }}>{i.marketplace.envRequired}</h3>
          <button className="btn btn-ghost btn-icon-sm" onClick={() => props.setShowEnvModal(null)}>
            <X size={16} />
          </button>
        </div>
        <p style={{ fontSize: 13, color: "var(--text-muted)", marginBottom: 16 }}>
          {locale === "zh"
            ? `安装 ${showEnvModal.name} 需要配置以下环境变量：`
            : `${showEnvModal.name} requires the following environment variables:`}
        </p>
        <div style={{ display: "flex", flexDirection: "column", gap: 12, marginBottom: 20 }}>
          {showEnvModal.env_keys.map((key) => (
            <div key={key}>
              <span className="field-label">{key}</span>
              <input
                className="input"
                style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 13 }}
                placeholder={`${locale === "zh" ? "输入" : "Enter"} ${key}`}
                value={envValues[key] || ""}
                onChange={(e) => props.setEnvValues((prev) => ({ ...prev, [key]: e.target.value }))}
              />
            </div>
          ))}
        </div>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button className="btn btn-secondary btn-sm" onClick={() => props.setShowEnvModal(null)}>
            {i.common.cancel}
          </button>
          <button className="btn btn-primary btn-sm" onClick={() => props.doInstallMcp(showEnvModal, envValues)}>
            <Download size={13} />
            {i.marketplace.install}
          </button>
        </div>
      </div>
    </div>
  );
}

export interface CustomSourceModalProps {
  locale: string;
  show: boolean;
  customUrl: string;
  loadingCustom: boolean;
  customSources: { url: string; count: number; skillIds: string[] }[];
  loadingRepo: string | null;
  recommendedRepos: { name: string; branch: string; desc: string }[];
  setShow: (v: boolean) => void;
  handleCustomUrlChange: (e: ChangeEvent<HTMLInputElement>) => void;
  handleCustomUrlKeyDown: (e: KeyboardEvent<HTMLInputElement>) => void;
  handleCustomSource: () => void;
  handleOpenRecommendedRepo: (name: string) => void;
  handleLoadRecommendedRepo: (name: string, branch: string) => void;
  removeCustomSource: (idx: number) => void;
  onSkillsLoaded: Dispatch<SetStateAction<SkillEntry[]>>;
}

interface SkillRepo {
  owner: string;
  name: string;
  branch: string;
  enabled: boolean;
}

function mergeDiscoveredSkills(previous: SkillEntry[], discovered: SkillEntry[]) {
  const byId = new Map(previous.map((skill) => [skill.id, skill]));
  for (const skill of discovered) byId.set(skill.id, skill);
  return Array.from(byId.values());
}

export function CustomSourceModal(props: CustomSourceModalProps) {
  const { locale, show, customUrl, loadingCustom, customSources, loadingRepo, recommendedRepos } = props;
  const { onSkillsLoaded } = props;
  const [repos, setRepos] = useState<SkillRepo[]>([]);
  const [repoOwner, setRepoOwner] = useState("");
  const [repoName, setRepoName] = useState("");
  const [repoBranch, setRepoBranch] = useState("main");
  const [repoBusy, setRepoBusy] = useState(false);
  const [repoError, setRepoError] = useState<string | null>(null);
  const [repoSkillCount, setRepoSkillCount] = useState(0);

  const reloadRepos = useCallback(async () => {
    setRepoError(null);
    try {
      const saved = await invoke<SkillRepo[]>("get_skill_repos");
      setRepos(saved);
      const discovered = await invoke<SkillEntry[]>("discover_available_skills");
      setRepoSkillCount(discovered.length);
      onSkillsLoaded((previous) => mergeDiscoveredSkills(previous, discovered));
    } catch (error) {
      setRepoError(String(error));
    }
  }, [onSkillsLoaded]);

  useEffect(() => {
    if (show) void reloadRepos();
  }, [reloadRepos, show]);

  const saveRepo = async (repo: SkillRepo) => {
    setRepoBusy(true);
    setRepoError(null);
    try {
      await invoke("add_skill_repo", { repo });
      setRepoOwner("");
      setRepoName("");
      setRepoBranch("main");
      await reloadRepos();
    } catch (error) {
      setRepoError(String(error));
    } finally {
      setRepoBusy(false);
    }
  };

  const removeRepo = async (repo: SkillRepo) => {
    setRepoBusy(true);
    setRepoError(null);
    try {
      await invoke("remove_skill_repo", { owner: repo.owner, name: repo.name });
      await reloadRepos();
    } catch (error) {
      setRepoError(String(error));
    } finally {
      setRepoBusy(false);
    }
  };

  const toggleRepo = (repo: SkillRepo) => void saveRepo({ ...repo, enabled: !repo.enabled });

  if (!show) return null;
  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "var(--bg-overlay)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
      onClick={() => props.setShow(false)}
    >
      <div
        className="section-card"
        style={{ width: 520, maxHeight: "80vh", overflowY: "auto" }}
        onClick={(e) => e.stopPropagation()}
      >
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 20 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <Globe size={18} style={{ color: "var(--text-secondary)" }} />
            <h3 style={{ fontSize: 16, fontWeight: 700 }}>{locale === "zh" ? "自定义源管理" : "Custom Sources"}</h3>
          </div>
          <button className="btn btn-ghost btn-icon-sm" onClick={() => props.setShow(false)}>
            <X size={16} />
          </button>
        </div>

        <div style={{ marginBottom: 20 }}>
          <span className="field-label">{locale === "zh" ? "内置源" : "Built-in Sources"}</span>
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 10,
                padding: "10px 14px",
                borderRadius: 8,
                background: "var(--bg-input)",
              }}
            >
              <Plug size={14} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
              <div style={{ minWidth: 0, flex: 1 }}>
                <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>MCP Registry</div>
                <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
                  {locale === "zh" ? "内置精选列表 + npm 搜索" : "Curated list + npm search"}
                </div>
              </div>
              <span className="badge badge-success" style={{ fontSize: 10 }}>
                {locale === "zh" ? "默认" : "Default"}
              </span>
            </div>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 10,
                padding: "10px 14px",
                borderRadius: 8,
                background: "var(--bg-input)",
              }}
            >
              <Zap size={14} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
              <div style={{ minWidth: 0, flex: 1 }}>
                <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>Skills Registry</div>
                <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
                  {locale === "zh" ? "内置技能注册表" : "Built-in skills registry"}
                </div>
              </div>
              <span className="badge badge-success" style={{ fontSize: 10 }}>
                {locale === "zh" ? "默认" : "Default"}
              </span>
            </div>
          </div>
        </div>

        <div style={{ marginBottom: 20 }}>
          <span className="field-label">{locale === "zh" ? "推荐技能仓库" : "Recommended Skill Repos"}</span>
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {recommendedRepos.map((repo) => (
              <MarketplaceRecommendedRepoRow
                key={repo.name}
                repoName={repo.name}
                branch={repo.branch}
                description={repo.desc}
                isLoaded={customSources.some((source) => source.url === `github:${repo.name}`)}
                isLoading={loadingRepo === repo.name}
                openLabel="GitHub"
                loadLabel={locale === "zh" ? "加载技能" : "Load Skills"}
                loadingLabel={locale === "zh" ? "加载中" : "Loading"}
                loadedLabel={locale === "zh" ? "已加载" : "Loaded"}
                onOpen={props.handleOpenRecommendedRepo}
                onLoad={props.handleLoadRecommendedRepo}
              />
            ))}
          </div>
        </div>

        {customSources.length > 0 && (
          <div style={{ marginBottom: 20 }}>
            <span className="field-label">{locale === "zh" ? "已添加的源" : "Added Sources"}</span>
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              {customSources.map((source, idx) => (
                <MarketplaceCustomSourceRow
                  key={source.url}
                  index={idx}
                  url={source.url}
                  count={source.count}
                  countLabel={locale === "zh" ? "个技能" : "skills"}
                  removeTitle={locale === "zh" ? "移除" : "Remove"}
                  onRemove={props.removeCustomSource}
                />
              ))}
            </div>
          </div>
        )}

        <div style={{ marginBottom: 20 }}>
          <span className="field-label">{locale === "zh" ? "持久化技能仓库" : "Persistent Skill Repositories"}</span>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 10 }}>
            {locale === "zh"
              ? "仓库会保存到本地，并在技能市场刷新时自动发现。"
              : "Repositories are saved locally and discovered when the marketplace refreshes."}
          </p>
          <div style={{ display: "flex", gap: 6, marginBottom: 8 }}>
            <input
              className="input"
              placeholder="owner"
              value={repoOwner}
              onChange={(event) => setRepoOwner(event.target.value)}
            />
            <input
              className="input"
              placeholder="repository"
              value={repoName}
              onChange={(event) => setRepoName(event.target.value)}
            />
            <input
              className="input"
              style={{ maxWidth: 90 }}
              placeholder="main"
              value={repoBranch}
              onChange={(event) => setRepoBranch(event.target.value)}
            />
            <button
              className="btn btn-primary btn-sm"
              disabled={repoBusy || !repoOwner.trim() || !repoName.trim()}
              onClick={() =>
                void saveRepo({
                  owner: repoOwner.trim(),
                  name: repoName.trim(),
                  branch: repoBranch.trim() || "main",
                  enabled: true,
                })
              }
            >
              <Plus size={13} />
            </button>
          </div>
          {repos.map((repo) => (
            <div
              key={`${repo.owner}/${repo.name}`}
              style={{ display: "flex", alignItems: "center", gap: 8, padding: "6px 0", fontSize: 12 }}
            >
              <button
                className={`badge ${repo.enabled ? "badge-success" : "badge-muted"}`}
                onClick={() => toggleRepo(repo)}
                disabled={repoBusy}
              >
                {repo.enabled ? (locale === "zh" ? "启用" : "On") : locale === "zh" ? "停用" : "Off"}
              </button>
              <span style={{ flex: 1, fontFamily: "'JetBrains Mono', monospace" }}>
                {repo.owner}/{repo.name}@{repo.branch}
              </span>
              <button
                className="btn btn-ghost btn-icon-sm"
                title={locale === "zh" ? "移除" : "Remove"}
                onClick={() => void removeRepo(repo)}
                disabled={repoBusy}
              >
                <Trash2 size={13} />
              </button>
            </div>
          ))}
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              marginTop: 8,
              fontSize: 11,
              color: "var(--text-muted)",
            }}
          >
            <span>{locale === "zh" ? `已发现 ${repoSkillCount} 个技能` : `${repoSkillCount} skills discovered`}</span>
            <button className="btn btn-secondary btn-sm" onClick={() => void reloadRepos()} disabled={repoBusy}>
              {locale === "zh" ? "刷新" : "Refresh"}
            </button>
          </div>
          {repoError && <div style={{ color: "var(--danger)", fontSize: 11, marginTop: 6 }}>{repoError}</div>}
        </div>

        <div>
          <span className="field-label">{locale === "zh" ? "添加新源" : "Add New Source"}</span>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 10 }}>
            {locale === "zh"
              ? "输入一个返回技能列表 JSON 的 URL"
              : "Enter a URL that returns a JSON array of skill entries"}
          </p>
          <div style={{ display: "flex", gap: 8 }}>
            <input
              className="input"
              style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 12, flex: 1 }}
              placeholder="https://example.com/skills.json"
              value={customUrl}
              onChange={props.handleCustomUrlChange}
              onKeyDown={props.handleCustomUrlKeyDown}
            />
            <button
              className="btn btn-primary btn-sm"
              onClick={props.handleCustomSource}
              disabled={!customUrl.trim() || loadingCustom}
              style={{ flexShrink: 0 }}
            >
              {loadingCustom ? (
                <>
                  <div className="spinner" style={{ width: 14, height: 14 }} />
                  {locale === "zh" ? "加载中" : "Loading"}
                </>
              ) : (
                <>
                  <Plus size={13} />
                  {locale === "zh" ? "添加" : "Add"}
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export interface SkillPreviewModalProps {
  locale: string;
  previewSkill: SkillEntry | null;
  currentToolInstalledSkills: Set<string>;
  showTranslation: boolean;
  setPreviewSkill: (v: SkillEntry | null) => void;
  startSkillEdit: (skill: SkillEntry) => void;
  handleUninstallSkill: (skill: SkillEntry) => void;
  handleInstallSkill: (skill: SkillEntry) => void;
}

export function SkillPreviewModal(props: SkillPreviewModalProps) {
  const { locale, previewSkill, currentToolInstalledSkills, showTranslation } = props;
  const i = t();
  if (!previewSkill) return null;
  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "var(--bg-overlay)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
      onClick={() => props.setPreviewSkill(null)}
    >
      <div
        className="section-card"
        style={{ width: 720, maxWidth: 720, maxHeight: "85vh", display: "flex", flexDirection: "column" }}
        onClick={(e) => e.stopPropagation()}
      >
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 18 }}>
          <div>
            <h3 style={{ fontSize: 16, fontWeight: 700 }}>{previewSkill.name}</h3>
            <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>
              {showTranslation && locale === "zh" && previewSkill.description_zh
                ? previewSkill.description_zh
                : previewSkill.description}
            </p>
          </div>
          <button className="btn btn-ghost btn-icon-sm" onClick={() => props.setPreviewSkill(null)}>
            <X size={16} />
          </button>
        </div>
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginBottom: 16 }}>
          <span className="badge badge-muted">{previewSkill.category}</span>
          {previewSkill.author && <span className="badge badge-muted">{previewSkill.author}</span>}
          {previewSkill.tags.map((tag) => (
            <span key={tag} className="badge badge-muted" style={{ fontSize: 10 }}>
              {tag}
            </span>
          ))}
          {previewSkill.github_url && (
            <button
              className="badge badge-accent"
              style={{
                fontSize: 10,
                display: "flex",
                alignItems: "center",
                gap: 3,
                cursor: "pointer",
                border: "none",
                background: "var(--accent-subtle)",
              }}
              onClick={() => shellOpen(previewSkill.github_url!)}
            >
              <ExternalLink size={10} />
              GitHub
            </button>
          )}
        </div>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
          <span className="field-label" style={{ marginBottom: 0 }}>
            {locale === "zh" ? "技能内容" : "Content"}
          </span>
          {currentToolInstalledSkills.has(previewSkill.name.toLowerCase()) && (
            <button
              className="btn btn-secondary btn-xs"
              onClick={() => void props.startSkillEdit(previewSkill)}
              style={{ gap: 5 }}
            >
              <Edit3 size={12} />
              {locale === "zh" ? "编辑" : "Edit"}
            </button>
          )}
        </div>
        <div
          className="markdown-preview"
          style={{ flex: 1, overflowY: "auto", fontSize: 13, lineHeight: 1.8, minHeight: 200 }}
        >
          <MarkdownPreview content={previewSkill.content} />
        </div>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
          <button className="btn btn-secondary btn-sm" onClick={() => props.setPreviewSkill(null)}>
            {i.common.cancel}
          </button>
          {currentToolInstalledSkills.has(previewSkill.name.toLowerCase()) ? (
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <button
                className="btn btn-danger-ghost btn-sm"
                onClick={() => {
                  props.handleUninstallSkill(previewSkill);
                  props.setPreviewSkill(null);
                }}
                style={{ gap: 5 }}
              >
                <Trash2 size={13} />
                {locale === "zh" ? "卸载" : "Uninstall"}
              </button>
              <span
                className="badge badge-success"
                style={{ display: "flex", alignItems: "center", gap: 4, padding: "6px 12px" }}
              >
                <CheckCircle size={12} />
                {i.marketplace.installed}
              </span>
            </div>
          ) : (
            <button
              className="btn btn-primary btn-sm"
              onClick={() => {
                props.handleInstallSkill(previewSkill);
                props.setPreviewSkill(null);
              }}
            >
              <Download size={13} />
              {i.marketplace.install}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

export interface McpPreviewModalProps {
  locale: string;
  previewMcp: RegistryEntry | null;
  currentToolInstalledIds: Set<string>;
  setPreviewMcp: (v: RegistryEntry | null) => void;
  startMcpEdit: (entry: RegistryEntry) => void;
  handleInstallMcp: (entry: RegistryEntry) => void;
}

export function McpPreviewModal(props: McpPreviewModalProps) {
  const { locale, previewMcp, currentToolInstalledIds } = props;
  const i = t();
  if (!previewMcp) return null;
  const isInstalled = currentToolInstalledIds.has(previewMcp.name) || currentToolInstalledIds.has(previewMcp.id);
  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "var(--bg-overlay)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
      onClick={() => props.setPreviewMcp(null)}
    >
      <div
        className="section-card"
        style={{ width: 560, maxHeight: "80vh", overflowY: "auto" }}
        onClick={(e) => e.stopPropagation()}
      >
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 18 }}>
          <h3 style={{ fontSize: 16, fontWeight: 700 }}>{previewMcp.name}</h3>
          <button className="btn btn-ghost btn-icon-sm" onClick={() => props.setPreviewMcp(null)}>
            <X size={16} />
          </button>
        </div>
        <p style={{ fontSize: 13, color: "var(--text-secondary)", marginBottom: 16, lineHeight: 1.6 }}>
          {previewMcp.description}
        </p>
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginBottom: 16 }}>
          <span className="badge badge-muted">
            {locale === "zh" ? MCP_CATEGORY_ZH[previewMcp.category] || previewMcp.category : previewMcp.category}
          </span>
          {previewMcp.install_type && <span className="badge badge-muted">{previewMcp.install_type}</span>}
          {previewMcp.package_name && (
            <span className="badge badge-accent" style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 10 }}>
              {previewMcp.package_name}
            </span>
          )}
        </div>
        <div style={{ marginBottom: 16 }}>
          <span className="field-label">{locale === "zh" ? "安装命令" : "Command"}</span>
          <div
            style={{
              padding: "10px 14px",
              borderRadius: 6,
              background: "var(--bg-input)",
              border: "1px solid var(--border-default)",
              fontFamily: "'JetBrains Mono', monospace",
              fontSize: 12,
              color: "var(--text-secondary)",
              wordBreak: "break-all",
            }}
          >
            {previewMcp.command} {previewMcp.args.join(" ")}
          </div>
        </div>
        {previewMcp.env_keys.length > 0 && (
          <div style={{ marginBottom: 16 }}>
            <span className="field-label">{locale === "zh" ? "需要的环境变量" : "Required Environment Variables"}</span>
            <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
              {previewMcp.env_keys.map((key) => (
                <span
                  key={key}
                  className="badge badge-warning"
                  style={{ fontSize: 10, display: "flex", alignItems: "center", gap: 3 }}
                >
                  <Key size={10} />
                  {key}
                </span>
              ))}
            </div>
          </div>
        )}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 8 }}>
          {previewMcp.github_url && (
            <button
              className="btn btn-secondary btn-sm"
              onClick={() => shellOpen(previewMcp.github_url!)}
              style={{ gap: 5 }}
            >
              <ExternalLink size={13} />
              GitHub
            </button>
          )}
          <button className="btn btn-secondary btn-sm" onClick={() => props.setPreviewMcp(null)}>
            {locale === "zh" ? "关闭" : "Close"}
          </button>
          {isInstalled ? (
            <>
              <button
                className="btn btn-secondary btn-sm"
                onClick={() => void props.startMcpEdit(previewMcp)}
                style={{ gap: 5 }}
              >
                <Edit3 size={13} />
                {locale === "zh" ? "编辑" : "Edit"}
              </button>
              <span
                className="badge badge-success"
                style={{ display: "flex", alignItems: "center", gap: 4, padding: "6px 12px" }}
              >
                <CheckCircle size={12} />
                {i.marketplace.installed}
              </span>
            </>
          ) : (
            <button
              className="btn btn-primary btn-sm"
              onClick={() => {
                props.handleInstallMcp(previewMcp);
                props.setPreviewMcp(null);
              }}
              style={{ gap: 5 }}
            >
              <Download size={13} />
              {i.marketplace.install}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

export interface UninstallSkillDialogProps {
  locale: string;
  pendingUninstall: SkillEntry | null;
  setPendingUninstall: (v: SkillEntry | null) => void;
  doUninstallSkill: (skill: SkillEntry) => void;
}

export function UninstallSkillDialog(props: UninstallSkillDialogProps) {
  const { locale, pendingUninstall } = props;
  return (
    <ConfirmDialog
      isOpen={!!pendingUninstall}
      title={locale === "zh" ? "卸载技能" : "Uninstall Skill"}
      message={
        locale === "zh" ? `确定卸载技能「${pendingUninstall?.name}」？` : `Uninstall skill "${pendingUninstall?.name}"?`
      }
      confirmText={locale === "zh" ? "卸载" : "Uninstall"}
      variant="destructive"
      onConfirm={() => {
        if (pendingUninstall) void props.doUninstallSkill(pendingUninstall);
        props.setPendingUninstall(null);
      }}
      onCancel={() => props.setPendingUninstall(null)}
    />
  );
}

export type { ToastFn };
