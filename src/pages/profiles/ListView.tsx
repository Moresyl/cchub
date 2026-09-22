/* eslint-disable @typescript-eslint/no-explicit-any */
import { Suspense, lazy, useState, type ChangeEvent } from "react";
import { ArrowRightLeft, ChevronDown, Monitor, Plus, RefreshCw, Search, X, type LucideIcon } from "lucide-react";

import ProfileCard from "../../components/ProfileCard";
import ProfileToolFilterTab from "../../components/ProfileToolFilterTab";
import LoadingState from "../../components/states/LoadingState";
import EmptyState from "../../components/states/EmptyState";
import UniversalProviderManager from "../../components/UniversalProviderManager";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";

import {
  TOOL_ICONS,
  extractConfigSummary,
  type ConfigProfile,
  type DetectedTool,
  type ProviderPingResult,
  type ProviderStreamCheckResult,
} from "./helpers";

const HermesProvidersPanel = lazy(() => import("../HermesProviders"));

type LocaleText = (zh: string, en: string, ja?: string) => string;

interface ProfilesListViewProps {
  locale: string;
  localeText: LocaleText;
  profiles: ConfigProfile[];
  activeIds: string[];
  tools: DetectedTool[];
  installedTools: DetectedTool[];
  toolCounts: Record<string, number>;
  filterTool: string;
  filteredProfiles: ConfigProfile[];
  activeIdSet: Set<string>;
  pingResults: Record<string, ProviderPingResult>;
  streamCheckResults: Record<string, ProviderStreamCheckResult>;
  sharedGroupCounts: Record<string, number>;
  search: string;
  searchInputRef: React.Ref<HTMLInputElement>;
  reorderEnabled: boolean;
  draggingProfileId: string | null;
  dragOverProfileId: string | null;
  pingingId: string | null;
  streamCheckingId: string | null;
  batchStreamChecking: boolean;
  applying: string | null;
  profileCardText: any;
  handleRefreshProfiles: () => void;
  handleOpenCreateProfile: () => void;
  handleSearchChange: (event: ChangeEvent<HTMLInputElement>) => void;
  handleClearSearch: () => void;
  handleToggleFilterTool: (toolId: string) => void;
  handleCardDragStart: (profileId: string) => void;
  handleCardDragEnter: (profileId: string) => void;
  handleCardDragEnd: () => void;
  handleCardDrop: (profileId: string) => void;
  handlePing: (profile: ConfigProfile) => void;
  handleStreamCheck: (profile: ConfigProfile) => void;
  handleUsage: (profile: ConfigProfile) => void;
  handleStreamCheckAll: () => void;
  doApply: (profile: ConfigProfile) => void;
  handleDuplicate: (profile: ConfigProfile) => void;
  openEditModal: (profile: ConfigProfile) => void;
  handleDelete: (profile: ConfigProfile) => void;
}

function pingToneFor(status: string | undefined) {
  if (status === "fast") return "badge-success";
  if (status === "medium") return "badge-warning";
  if (status === "slow" || status === "error") return "badge-danger";
  return "badge-muted";
}

function streamToneFor(status: string | undefined) {
  if (status === "healthy") return "badge-success";
  if (status === "reachable") return "badge-warning";
  if (status === "unsupported" || status === "unconfigured") return "badge-muted";
  if (!status) return "badge-muted";
  return "badge-danger";
}

export default function ProfilesListView(props: ProfilesListViewProps) {
  const { locale, localeText, profiles, activeIds, tools, installedTools, toolCounts, filterTool } = props;
  const [showSharedProviders, setShowSharedProviders] = useState(false);
  return (
    <>
      <div className="page-header profile-page-header">
        <div>
          <h2 className="page-title">{locale === "zh" ? "配置切换" : "Config Profiles"}</h2>
          <p className="page-subtitle">
            {locale === "zh"
              ? `共 ${profiles.length} 个配置，当前生效 ${activeIds.length} 个`
              : `${profiles.length} profiles, ${activeIds.length} active`}
          </p>
        </div>
        {filterTool !== "hermes" && (
          <div style={{ display: "flex", gap: 8 }}>
            <Button variant="secondary" size="sm" onClick={props.handleRefreshProfiles}>
              <RefreshCw size={14} />
              {locale === "zh" ? "刷新" : "Refresh"}
            </Button>
            <Button
              variant="secondary"
              size="sm"
              onClick={props.handleStreamCheckAll}
              disabled={props.batchStreamChecking}
              style={{ gap: 6 }}
            >
              <RefreshCw size={14} />
              {props.batchStreamChecking
                ? localeText("检查中...", "Checking...", "確認中...")
                : localeText("全量流检", "Check all streams", "全体ストリーム確認")}
            </Button>
            <Button
              size="sm"
              onClick={props.handleOpenCreateProfile}
              disabled={installedTools.length === 0}
              style={{ gap: 6 }}
            >
              <Plus size={14} />
              {locale === "zh" ? "新增" : "New"}
            </Button>
          </div>
        )}
      </div>

      <div className="profile-toolbar">
        <div style={{ position: "relative", flex: 1, minWidth: 240, maxWidth: 360 }}>
          <Search
            size={14}
            style={{
              position: "absolute",
              left: 12,
              top: "50%",
              transform: "translateY(-50%)",
              color: "var(--text-muted)",
            }}
          />
          <Input
            ref={props.searchInputRef}
            className="input"
            style={{ paddingLeft: 36 }}
            placeholder={localeText("搜索配置...", "Search...", "設定を検索...")}
            value={props.search}
            onChange={props.handleSearchChange}
          />
          {props.search && (
            <Button
              variant="ghost"
              size="icon"
              aria-label={locale === "zh" ? "清除搜索" : "Clear search"}
              title={locale === "zh" ? "清除搜索" : "Clear search"}
              style={{ position: "absolute", right: 6, top: "50%", transform: "translateY(-50%)" }}
              onClick={props.handleClearSearch}
            >
              <X size={14} />
            </Button>
          )}
        </div>
        <div className="profile-tool-tabs" role="tablist" aria-label={localeText("工具", "Tools", "ツール")}>
          {tools.map((tool) => (
            <ProfileToolFilterTab
              key={tool.id}
              toolId={tool.id}
              toolName={tool.name}
              count={toolCounts[tool.id] || 0}
              active={filterTool === tool.id}
              dimmed={!tool.installed && (toolCounts[tool.id] || 0) === 0}
              onToggle={props.handleToggleFilterTool}
            />
          ))}
        </div>
      </div>

      <Button
        variant="ghost"
        size="sm"
        className="profile-shared-toggle"
        aria-expanded={showSharedProviders}
        onClick={() => setShowSharedProviders((value) => !value)}
      >
        <ChevronDown size={14} className={showSharedProviders ? "profile-chevron-open" : ""} />
        {localeText("跨工具共享配置", "Shared providers", "共有プロバイダー")}
      </Button>
      {showSharedProviders && (
        <UniversalProviderManager
          locale={locale}
          localeText={localeText}
          onProfilesChanged={props.handleRefreshProfiles}
        />
      )}

      {filterTool === "hermes" && (
        <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
          <Suspense fallback={<LoadingState />}>
            <HermesProvidersPanel embedded />
          </Suspense>
        </div>
      )}

      <div
        className="profile-list"
        style={{
          flex: 1,
          minHeight: 0,
          overflowY: "auto",
          display: filterTool === "hermes" ? "none" : "flex",
          flexDirection: "column",
          gap: 10,
        }}
      >
        {props.filteredProfiles.length === 0 ? (
          <EmptyState
            icon={<ArrowRightLeft size={28} style={{ color: "var(--text-muted)" }} />}
            title={locale === "zh" ? "没有可显示的配置" : "No configurations to display"}
            description={
              locale === "zh"
                ? "点击右上角「新增」保存一份当前配置，之后就可以在这里一键切换。"
                : 'Click "New" to save a configuration, then switch here.'
            }
          />
        ) : (
          props.filteredProfiles.map((profile) => {
            const Icon: LucideIcon = TOOL_ICONS[profile.tool_id] || Monitor;
            const isActive = props.activeIdSet.has(profile.id);
            const summary = extractConfigSummary(profile.tool_id, profile.config_snapshot);
            const ping = props.pingResults[profile.id];
            const streamCheck = props.streamCheckResults[profile.id];
            const sharedCount =
              profile.source_type === "shared" && profile.source_key
                ? props.sharedGroupCounts[profile.source_key] || 1
                : 0;
            return (
              <ProfileCard
                key={profile.id}
                profile={profile}
                icon={Icon}
                isActive={isActive}
                toolTag={profile.tool_id}
                iconUrl={summary.iconUrl}
                sharedTag={localeText(
                  `共享 ${sharedCount} App`,
                  `Shared ${sharedCount} apps`,
                  `${sharedCount} App 共有`,
                )}
                baseUrl={summary.baseUrl}
                model={summary.model}
                sharedCount={sharedCount}
                ping={ping}
                pingTone={pingToneFor(ping?.status)}
                streamCheck={streamCheck}
                streamTone={streamToneFor(streamCheck?.status)}
                reorderEnabled={props.reorderEnabled}
                isDragging={props.draggingProfileId === profile.id}
                isDragOver={props.dragOverProfileId === profile.id}
                isPinging={props.pingingId === profile.id}
                isStreamChecking={props.streamCheckingId === profile.id}
                isApplying={props.applying === profile.id}
                text={props.profileCardText}
                onDragStart={props.handleCardDragStart}
                onDragEnter={props.handleCardDragEnter}
                onDragEnd={props.handleCardDragEnd}
                onDrop={props.handleCardDrop}
                onPing={props.handlePing}
                onStreamCheck={props.handleStreamCheck}
                onUsage={props.handleUsage}
                onApply={props.doApply}
                onDuplicate={props.handleDuplicate}
                onEdit={props.openEditModal}
                onDelete={props.handleDelete}
              />
            );
          })
        )}
      </div>
    </>
  );
}
