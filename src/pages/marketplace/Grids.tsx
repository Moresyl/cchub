/* eslint-disable @typescript-eslint/no-explicit-any */
import { Store, Zap } from "lucide-react";

import EmptyState from "../../components/states/EmptyState";
import FeaturedSkillBundleCard, { type FeaturedSkillBundle } from "../../components/FeaturedSkillBundleCard";
import MarketplaceMcpCard from "../../components/MarketplaceMcpCard";
import MarketplaceSkillCard from "../../components/MarketplaceSkillCard";
import { t } from "../../lib/i18n";

import { MCP_CATEGORY_ZH, type RegistryEntry, type SkillEntry } from "./helpers";

export interface McpGridProps {
  locale: string;
  filteredMcp: RegistryEntry[];
  currentToolInstalledIds: Set<string>;
  installing: string | null;
  activeTool: string;
  mcpCategory: string;
  mcpTotal: number;
  mcpPage: number;
  loadingMore: boolean;
  handlePreviewMcp: (entry: RegistryEntry) => void;
  handleInstallMcpCard: (entry: RegistryEntry) => void;
  handleEditMcp: (entry: RegistryEntry) => void;
  handleUninstallMcpCard: (entry: RegistryEntry) => void;
  handleOpenGithub: (url: string) => void;
  handleLoadPrevMcpPage: () => void;
  handleLoadNextMcpPage: () => void;
}

export function McpGrid(props: McpGridProps) {
  const {
    locale,
    filteredMcp,
    currentToolInstalledIds,
    installing,
    activeTool,
    mcpCategory,
    mcpTotal,
    mcpPage,
    loadingMore,
  } = props;
  const i = t();
  if (filteredMcp.length === 0) {
    return (
      <EmptyState
        icon={<Store size={28} style={{ color: "var(--text-muted)" }} />}
        title={i.marketplace.noResults}
        description={i.marketplace.noResultsTip}
      />
    );
  }
  return (
    <>
      <div
        style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(320px, 1fr))", gap: 16 }}
        className="stagger"
      >
        {filteredMcp.map((entry) => {
          const isInstalled = currentToolInstalledIds.has(entry.name) || currentToolInstalledIds.has(entry.id);
          const isInstalling = installing === entry.id;
          return (
            <MarketplaceMcpCard
              key={entry.id}
              entry={entry}
              categoryLabel={locale === "zh" ? MCP_CATEGORY_ZH[entry.category] || entry.category : entry.category}
              installed={isInstalled}
              installing={isInstalling}
              installedLabel={i.marketplace.installed}
              installLabel={i.marketplace.install}
              installingLabel={i.marketplace.installing}
              editTitle={locale === "zh" ? "编辑" : "Edit"}
              uninstallTitle={locale === "zh" ? `从 ${activeTool} 卸载` : `Remove from ${activeTool}`}
              githubLabel="GitHub"
              keysLabel={locale === "zh" ? "密钥" : "keys"}
              onPreview={props.handlePreviewMcp}
              onInstall={props.handleInstallMcpCard}
              onEdit={props.handleEditMcp}
              onUninstall={props.handleUninstallMcpCard}
              onOpenGithub={props.handleOpenGithub}
            />
          );
        })}
      </div>
      {mcpCategory !== "installed" && mcpTotal > 50 && (
        <div style={{ display: "flex", justifyContent: "center", alignItems: "center", gap: 12, padding: "20px 0" }}>
          <button
            className="btn btn-secondary btn-sm"
            disabled={mcpPage === 0 || loadingMore}
            onClick={() => void props.handleLoadPrevMcpPage()}
          >
            {locale === "zh" ? "上一页" : "Prev"}
          </button>
          <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
            {locale === "zh"
              ? `第 ${mcpPage + 1} / ${Math.ceil(mcpTotal / 50)} 页（共 ${mcpTotal} 个）`
              : `Page ${mcpPage + 1} / ${Math.ceil(mcpTotal / 50)} (${mcpTotal} total)`}
          </span>
          <button
            className="btn btn-secondary btn-sm"
            disabled={(mcpPage + 1) * 50 >= mcpTotal || loadingMore}
            onClick={() => void props.handleLoadNextMcpPage()}
          >
            {loadingMore ? <div className="spinner" style={{ width: 12, height: 12 }} /> : null}
            {locale === "zh" ? "下一页" : "Next"}
          </button>
        </div>
      )}
    </>
  );
}

export interface SkillsGridProps {
  locale: string;
  filteredSkills: SkillEntry[];
  currentToolInstalledSkills: Set<string>;
  installing: string | null;
  activeTool: string;
  showTranslation: boolean;
  featuredBundles: FeaturedSkillBundle[];
  installingBundle: string | null;
  bundleInstalledCount: (bundle: FeaturedSkillBundle) => number;
  handleInstallBundle: (bundle: FeaturedSkillBundle) => void;
  handleOpenGithub: (url: string) => void;
  handleOpenSkillPreview: (skill: SkillEntry) => void;
  handleInstallMarketSkill: (skill: SkillEntry) => void;
  handleEditMarketSkill: (skill: SkillEntry) => void;
  handleUninstallMarketSkill: (skill: SkillEntry) => void;
}

export function SkillsGrid(props: SkillsGridProps) {
  const {
    locale,
    filteredSkills,
    currentToolInstalledSkills,
    installing,
    activeTool,
    showTranslation,
    featuredBundles,
    installingBundle,
  } = props;
  const i = t();
  return (
    <>
      {featuredBundles.length > 0 &&
        (() => {
          const sorted = [...featuredBundles].sort((a, b) => {
            const aFull = props.bundleInstalledCount(a) >= a.totalSkills;
            const bFull = props.bundleInstalledCount(b) >= b.totalSkills;
            if (aFull === bFull) return 0;
            return aFull ? 1 : -1;
          });
          return (
            <div style={{ marginBottom: 20 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 10 }}>
                <span
                  style={{
                    fontSize: 11,
                    fontWeight: 700,
                    letterSpacing: "0.06em",
                    textTransform: "uppercase",
                    color: "var(--text-muted)",
                  }}
                >
                  {locale === "zh" ? "精选打包" : locale === "ja" ? "厳選パッケージ" : "Featured bundles"}
                </span>
              </div>
              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(360px, 1fr))", gap: 16 }}>
                {sorted.map((bundle) => {
                  const installedCount = props.bundleInstalledCount(bundle);
                  const fullyInstalled = installedCount >= bundle.totalSkills;
                  const isInstalling = installingBundle === bundle.id;
                  return (
                    <FeaturedSkillBundleCard
                      key={bundle.id}
                      bundle={bundle}
                      fullyInstalled={fullyInstalled}
                      installing={isInstalling}
                      installedCount={installedCount}
                      installAllLabel={
                        locale === "zh" ? "一键安装全部" : locale === "ja" ? "すべてインストール" : "Install all"
                      }
                      installingLabel={locale === "zh" ? "安装中" : locale === "ja" ? "インストール中" : "Installing"}
                      installedLabel={locale === "zh" ? "已安装" : locale === "ja" ? "インストール済み" : "Installed"}
                      reinstallLabel={locale === "zh" ? "重新安装" : locale === "ja" ? "再インストール" : "Reinstall"}
                      bundleBadgeLabel={locale === "zh" ? "精选" : locale === "ja" ? "厳選" : "Featured"}
                      githubLabel="GitHub"
                      onInstallAll={props.handleInstallBundle}
                      onOpenGithub={props.handleOpenGithub}
                    />
                  );
                })}
              </div>
            </div>
          );
        })()}

      {filteredSkills.length === 0 ? (
        <EmptyState
          icon={<Zap size={28} style={{ color: "var(--text-muted)" }} />}
          title={locale === "zh" ? "未找到技能" : "No skills found"}
          description={locale === "zh" ? "尝试其他关键词或添加自定义源" : "Try different keywords or add custom source"}
        />
      ) : (
        <div
          style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(320px, 1fr))", gap: 16 }}
          className="stagger"
        >
          {filteredSkills.map((skill) => {
            const isInstalled = currentToolInstalledSkills.has(skill.name.toLowerCase());
            const isInstalling = installing === skill.id;
            const desc =
              showTranslation && locale === "zh" && skill.description_zh ? skill.description_zh : skill.description;
            return (
              <MarketplaceSkillCard
                key={skill.id}
                skill={skill}
                description={desc}
                installed={isInstalled}
                installing={isInstalling}
                installedLabel={i.marketplace.installed}
                installLabel={i.marketplace.install}
                installingLabel={i.marketplace.installing}
                editTitle={locale === "zh" ? "编辑" : "Edit"}
                uninstallTitle={locale === "zh" ? `从 ${activeTool} 卸载` : `Remove from ${activeTool}`}
                githubLabel="GitHub"
                onPreview={props.handleOpenSkillPreview}
                onInstall={props.handleInstallMarketSkill}
                onEdit={props.handleEditMarketSkill}
                onUninstall={props.handleUninstallMarketSkill}
                onOpenGithub={props.handleOpenGithub}
              />
            );
          })}
        </div>
      )}
    </>
  );
}
