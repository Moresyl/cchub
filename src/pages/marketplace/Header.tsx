/* eslint-disable @typescript-eslint/no-explicit-any */
import { Monitor, Plug, Plus, Search, X, Zap } from "lucide-react";
import type { ChangeEvent, KeyboardEvent, Ref } from "react";

import MarketplaceCategoryTab from "../../components/MarketplaceCategoryTab";
import { t } from "../../lib/i18n";
import type { DetectedTool } from "../../types/skills";

import { TOOL_ICONS } from "./helpers";

type LocaleText = (zh: string, en: string, ja?: string) => string;

export interface MarketplaceHeaderProps {
  locale: string;
  localeText: LocaleText;
  tab: "mcp" | "skills";
  search: string;
  activeTool: string;
  visibleTools: DetectedTool[];
  entriesCount: number;
  skillEntriesCount: number;
  activeCategoryKey: string;
  currentCategories: { key: string; label: string }[];
  searchInputRef: Ref<HTMLInputElement>;
  setShowCustomSource: (v: boolean) => void;
  setActiveTool: (v: string) => void;
  setTab: (v: "mcp" | "skills") => void;
  setSearch: (v: string) => void;
  setPreviewSkill: (v: null) => void;
  setPreviewMcp: (v: null) => void;
  handleSearchChange: (e: ChangeEvent<HTMLInputElement>) => void;
  handleSearchKeyDown: (e: KeyboardEvent<HTMLInputElement>) => void;
  handleClearSearch: () => void;
  handleSearch: () => void;
  handleSelectCategory: (key: string) => void;
}

export function MarketplaceHeader(props: MarketplaceHeaderProps) {
  const {
    locale,
    localeText,
    tab,
    search,
    activeTool,
    visibleTools,
    entriesCount,
    skillEntriesCount,
    activeCategoryKey,
    currentCategories,
  } = props;
  const i = t();
  return (
    <>
      <div className="page-header">
        <div>
          <h2 className="page-title">{i.marketplace.title}</h2>
          <p className="page-subtitle">{i.marketplace.subtitle}</p>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          {tab === "skills" && (
            <button
              className="btn btn-secondary btn-sm"
              onClick={() => props.setShowCustomSource(true)}
              style={{ gap: 6 }}
            >
              <Plus size={14} />
              {locale === "zh" ? "自定义源" : "Custom Source"}
            </button>
          )}
        </div>
      </div>

      {visibleTools.length > 0 && (
        <div style={{ display: "flex", gap: 8, marginBottom: 16, flexWrap: "wrap", alignItems: "center" }}>
          {visibleTools.map((tool) => {
            const Icon = TOOL_ICONS[tool.id] || Monitor;
            const isActive = activeTool === tool.id;
            return (
              <div key={tool.id} style={{ position: "relative" }}>
                <button
                  className={`btn btn-sm ${isActive ? "btn-primary" : tool.installed ? "btn-secondary" : "btn-ghost"}`}
                  onClick={() => {
                    if (!tool.installed) return;
                    props.setActiveTool(tool.id);
                    props.setSearch("");
                    props.setPreviewSkill(null);
                    props.setPreviewMcp(null);
                  }}
                  style={{
                    gap: 6,
                    opacity: tool.installed ? 1 : 0.5,
                    cursor: tool.installed ? "pointer" : "default",
                  }}
                  title={
                    tool.installed ? tool.name : locale === "zh" ? `${tool.name} 未安装` : `${tool.name} not installed`
                  }
                >
                  <Icon size={14} />
                  {tool.name}
                  {!tool.installed && (
                    <span
                      style={{
                        width: 6,
                        height: 6,
                        borderRadius: "50%",
                        background: "var(--danger)",
                        flexShrink: 0,
                      }}
                    />
                  )}
                </button>
              </div>
            );
          })}
          {visibleTools.filter((tool) => !tool.installed).length > 0 && (
            <span style={{ fontSize: 11, color: "var(--text-muted)", marginLeft: 4 }}>
              {locale === "zh" ? "红点 = 未安装" : "red dot = not installed"}
            </span>
          )}
        </div>
      )}

      <div style={{ display: "flex", gap: 8, marginBottom: 16 }}>
        <button
          className={`btn btn-sm ${tab === "mcp" ? "btn-primary" : "btn-secondary"}`}
          onClick={() => {
            props.setTab("mcp");
            props.setSearch("");
          }}
          style={{ gap: 6 }}
        >
          <Plug size={14} />
          {locale === "zh" ? "MCP 服务" : "MCP Servers"}
          <span style={{ fontSize: 11, opacity: 0.7 }}>({entriesCount})</span>
        </button>
        <button
          className={`btn btn-sm ${tab === "skills" ? "btn-primary" : "btn-secondary"}`}
          onClick={() => {
            props.setTab("skills");
            props.setSearch("");
          }}
          style={{ gap: 6 }}
        >
          <Zap size={14} />
          {locale === "zh" ? "技能" : "Skills"}
          <span style={{ fontSize: 11, opacity: 0.7 }}>({skillEntriesCount})</span>
        </button>
      </div>

      <div style={{ display: "flex", gap: 16, marginBottom: 20, alignItems: "center" }}>
        <div style={{ position: "relative", flex: 1, maxWidth: 400, display: "flex", gap: 8 }}>
          <div style={{ position: "relative", flex: 1 }}>
            <Search
              size={15}
              style={{
                position: "absolute",
                left: 14,
                top: "50%",
                transform: "translateY(-50%)",
                color: "var(--text-muted)",
              }}
            />
            <input
              ref={props.searchInputRef}
              className="input"
              style={{ paddingLeft: 40, paddingRight: search ? 36 : 12 }}
              placeholder={
                tab === "mcp"
                  ? i.marketplace.searchPlaceholder
                  : localeText("搜索技能...", "Search skills...", "スキルを検索...")
              }
              value={search}
              onChange={props.handleSearchChange}
              onKeyDown={props.handleSearchKeyDown}
            />
            {search && (
              <button
                className="btn btn-ghost btn-icon-sm"
                aria-label={locale === "zh" ? "清除搜索" : "Clear search"}
                title={locale === "zh" ? "清除搜索" : "Clear search"}
                style={{ position: "absolute", right: 6, top: "50%", transform: "translateY(-50%)" }}
                onClick={props.handleClearSearch}
              >
                <X size={14} />
              </button>
            )}
          </div>
          <button className="btn btn-primary btn-sm" onClick={props.handleSearch} style={{ flexShrink: 0, gap: 5 }}>
            <Search size={13} />
            {locale === "zh" ? "搜索" : "Search"}
          </button>
        </div>
        <div className="tab-bar" style={{ flexWrap: "wrap" }}>
          {currentCategories.map((cat) => (
            <MarketplaceCategoryTab
              key={cat.key}
              categoryKey={cat.key}
              label={cat.label}
              active={activeCategoryKey === cat.key}
              onSelect={props.handleSelectCategory}
            />
          ))}
        </div>
      </div>
    </>
  );
}
