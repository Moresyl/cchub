import type { RefObject } from "react";

export interface CustomPath {
  tool_id: string;
  config_dir: string | null;
  mcp_config_path: string | null;
  skills_dir: string | null;
}

export interface DetectedTool {
  id: string;
  name: string;
  config_path: string;
  skills_dir: string;
  mcp_config_path: string;
  installed: boolean;
  install_command: string;
  install_url: string;
}

export interface PendingImportedProjectRoot {
  project_root: string;
  file_count: number;
}

export interface AutoRemapImportedProjectRootsResult {
  remapped_roots: number;
  restored_files: number;
  skipped_roots: number;
}

export interface LastImportSummary {
  imported_at: string;
  db_rows_restored: number;
  tool_configs_restored: number;
  skills_restored: number;
  full_files_restored: number;
  pending_project_files: number;
  safety_backup_path: string;
}

export interface FullRescanResult {
  mcp_servers: number;
  skills: number;
  hooks: number;
  instruction_files: number;
  workflows: number;
  config_roots: number;
  pending_project_roots: number;
  tool_health_issues: number;
  manual_setup_required: number;
}

export interface ToolEnvironmentReport {
  tool_id: string;
  tool_name: string;
  cli_available: boolean;
  cli_command: string;
  config_path: string;
  config_exists: boolean;
  mcp_config_path: string;
  mcp_config_exists: boolean;
  skills_dir: string;
  skills_dir_exists: boolean;
  config_dir: string;
  config_dir_exists: boolean;
  has_custom_config_dir: boolean;
  has_custom_mcp_config_path: boolean;
  has_custom_skills_dir: boolean;
  manual_setup_kind: string | null;
  manual_setup_command: string | null;
  manual_setup_path: string | null;
}

export interface BootstrapToolEnvironmentResult {
  created_dirs: number;
  created_files: number;
  notes: string[];
}

export interface RepairAllResult {
  remapped_roots: number;
  restored_project_files: number;
  skipped_remap_roots: number;
  bootstrapped_tools: number;
  created_dirs: number;
  created_files: number;
  bootstrap_notes: string[];
  rescan: FullRescanResult;
}

export interface ManagedBackupFile {
  path: string;
  name: string;
  created_at: string;
  size_bytes: number;
  kind: string;
  can_restore: boolean;
}

export interface BackupPreferences {
  auto_backup_enabled: boolean;
  retention_count: number;
}

export interface MigrationPanelState {
  summary: boolean;
  pending: boolean;
  health: boolean;
  auth: boolean;
}

export interface MigrationPanelRefs {
  summary: RefObject<HTMLDetailsElement | null>;
  pending: RefObject<HTMLDetailsElement | null>;
  health: RefObject<HTMLDetailsElement | null>;
  auth: RefObject<HTMLDetailsElement | null>;
}

export function hasToolHealthIssue(report: ToolEnvironmentReport) {
  return !report.cli_available
    || !report.config_dir_exists
    || !report.config_exists
    || !report.mcp_config_exists
    || !report.skills_dir_exists;
}
