/* eslint-disable @typescript-eslint/no-explicit-any */
import { Suspense, lazy, useState, type ChangeEvent } from "react";
import {
  ArrowRightLeft,
  ChevronDown,
  Info,
  Monitor,
  Plus,
  RefreshCw,
  Search,
  Wifi,
  X,
  type LucideIcon,
} from "lucide-react";

import ProfileCard from "../../components/ProfileCard";
import ProfileToolFilterTab from "../../components/ProfileToolFilterTab";
import LoadingState from "../../components/states/LoadingState";
import EmptyState from "../../components/states/EmptyState";
import UniversalProviderManager from "../../components/UniversalProviderManager";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import appIcon from "../../../src-tauri/icons/128x128.png";

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
  const activeTool = tools.find((tool) => tool.id === filterTool);
  const hour = new Date().getHours();
  const greeting =
    hour < 6
      ? localeText("夜深了", "Working late", "夜遅くまでお疲れさまです")
      : hour < 12
        ? localeText("早上好", "Good morning", "おはようございます")
        : hour < 18
          ? localeText("下午好", "Good afternoon", "こんにちは")
          : localeText("晚上好", "Good evening", "こんばんは");

  return (
    <section className="profile-workspace">
      <div className="profile-workspace-inner">
        <div className="profile-hero">
          <img className="profile-hero-mark" src={appIcon} alt="" aria-hidden="true" />
          <h2>
            {localeText(
              `${greeting}，选择一个配置继续`,
              `${greeting}. Pick a configuration`,
              `${greeting}。設定を選択`,
            )}
          </h2>
        </div>

        <div className="profile-status-bar">
          <Info size={14} aria-hidden="true" />
          <span>
            {localeText(
              `已保存 ${profiles.length} 个配置，其中 ${activeIds.length} 个正在生效`,
              `${profiles.length} configurations saved, ${activeIds.length} active`,
              `${profiles.length} 件の設定を保存済み、${activeIds.length} 件が有効`,
            )}
          </span>
        </div>

        <div className="profile-composer">
          <div className="profile-composer-context">
            <span>{activeTool?.name || localeText("选择工具", "Choose a tool", "ツールを選択")}</span>
            <span>
              {localeText(
                `${toolCounts[filterTool] || 0} 个配置`,
                `${toolCounts[filterTool] || 0} profiles`,
                `${toolCounts[filterTool] || 0} 件`,
              )}
            </span>
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

          <div className="profile-search">
            <Search size={16} aria-hidden="true" />
            <Input
              ref={props.searchInputRef}
              className="profile-search-input"
              placeholder={localeText("搜索要切换的配置...", "Search configurations...", "設定を検索...")}
              value={props.search}
              onChange={props.handleSearchChange}
            />
            {props.search && (
              <Button
                variant="ghost"
                size="icon"
                aria-label={locale === "zh" ? "清除搜索" : "Clear search"}
                title={locale === "zh" ? "清除搜索" : "Clear search"}
                onClick={props.handleClearSearch}
              >
                <X size={14} />
              </Button>
            )}
          </div>

          <div className="profile-composer-actions">
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
            {filterTool !== "hermes" && (
              <div className="profile-composer-action-group">
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={props.handleRefreshProfiles}
                  aria-label={locale === "zh" ? "刷新" : "Refresh"}
                  title={locale === "zh" ? "刷新" : "Refresh"}
                >
                  <RefreshCw size={14} />
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={props.handleStreamCheckAll}
                  disabled={props.batchStreamChecking}
                >
                  <Wifi size={14} />
                  {props.batchStreamChecking
                    ? localeText("检查中...", "Checking...", "確認中...")
                    : localeText("全量流检", "Check streams", "全体ストリーム確認")}
                </Button>
                <Button size="sm" onClick={props.handleOpenCreateProfile} disabled={installedTools.length === 0}>
                  <Plus size={14} />
                  {locale === "zh" ? "新增" : "New"}
                </Button>
              </div>
            )}
          </div>
        </div>

        {showSharedProviders && (
          <div className="profile-shared-panel">
            <UniversalProviderManager
              locale={locale}
              localeText={localeText}
              onProfilesChanged={props.handleRefreshProfiles}
            />
          </div>
        )}

        {filterTool === "hermes" && (
          <div className="profile-embedded-panel">
            <Suspense fallback={<LoadingState />}>
              <HermesProvidersPanel embedded />
            </Suspense>
          </div>
        )}

        {filterTool !== "hermes" && (
          <div className="profile-results">
            <div className="profile-results-header">
              <span>{localeText("可用配置", "Available configurations", "利用可能な設定")}</span>
              <span>{props.filteredProfiles.length}</span>
            </div>
            <div className="profile-list">
              {props.filteredProfiles.length === 0 ? (
                <EmptyState
                  icon={<ArrowRightLeft size={28} style={{ color: "var(--text-muted)" }} />}
                  title={locale === "zh" ? "没有可显示的配置" : "No configurations to display"}
                  description={
                    locale === "zh"
                      ? "点击「新增」保存当前配置，之后即可在这里一键切换。"
                      : 'Select "New" to save a configuration and switch to it here.'
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
          </div>
        )}
      </div>
    </section>
  );
}
