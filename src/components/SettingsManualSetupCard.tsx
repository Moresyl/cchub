import { memo } from "react";
import { Copy, FolderOpen, Link2, Loader2 } from "lucide-react";
import { Button } from "./ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";

export interface SettingsManualSetupCardReport {
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

interface SettingsManualSetupCardProps {
  report: SettingsManualSetupCardReport;
  description: string;
  installUrl: string | null;
  bootstrapping: boolean;
  copyCommandLabel: string;
  copyPathLabel: string;
  openPathLabel: string;
  prepareFileLabel: string;
  openDocsLabel: string;
  bootstrappingLabel: string;
  commandToastLabel: string;
  pathToastLabel: string;
  openPathToastLabel: string;
  docsToastLabel: string;
  onCopy: (value: string, label: string) => void | Promise<void>;
  onOpen: (target: string, label: string) => void | Promise<void>;
  onBootstrap: (toolId: string, toolName: string) => void | Promise<void>;
}

function SettingsManualSetupCardComponent({
  report,
  description,
  installUrl,
  bootstrapping,
  copyCommandLabel,
  copyPathLabel,
  openPathLabel,
  prepareFileLabel,
  openDocsLabel,
  bootstrappingLabel,
  commandToastLabel,
  pathToastLabel,
  openPathToastLabel,
  docsToastLabel,
  onCopy,
  onOpen,
  onBootstrap,
}: SettingsManualSetupCardProps) {
  return (
    <Card className="min-w-0">
      <CardHeader className="flex-row flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 flex-1 space-y-2">
          <CardTitle className="break-words text-[14px] leading-snug">{report.tool_name}</CardTitle>
          <CardDescription className="break-words">{description}</CardDescription>
        </div>
        <span className="badge badge-muted">{report.tool_id}</span>
      </CardHeader>
      <CardContent className="space-y-3">
        {report.manual_setup_path && (
          <div className="break-all font-mono text-[12px] leading-relaxed text-muted-foreground">
            {report.manual_setup_path}
          </div>
        )}
        <div className="flex flex-wrap gap-2">
          {report.manual_setup_command && (
            <Button
              type="button"
              variant="outline"
              onClick={() => onCopy(report.manual_setup_command || "", commandToastLabel)}
            >
              <Copy size={14} aria-hidden="true" />
              {copyCommandLabel}
            </Button>
          )}
          {report.manual_setup_path && (
            <Button
              type="button"
              variant="outline"
              onClick={() => onCopy(report.manual_setup_path || "", pathToastLabel)}
            >
              <Copy size={14} aria-hidden="true" />
              {copyPathLabel}
            </Button>
          )}
          {report.manual_setup_path && (
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpen(report.manual_setup_path || "", openPathToastLabel)}
            >
              <FolderOpen size={14} aria-hidden="true" />
              {openPathLabel}
            </Button>
          )}
          <Button
            type="button"
            onClick={() => onBootstrap(report.tool_id, report.tool_name)}
            disabled={bootstrapping}
            aria-busy={bootstrapping}
          >
            {bootstrapping ? (
              <Loader2 size={14} className="animate-spin motion-reduce:animate-none" aria-hidden="true" />
            ) : (
              <FolderOpen size={14} aria-hidden="true" />
            )}
            {bootstrapping ? bootstrappingLabel : prepareFileLabel}
          </Button>
          {installUrl && (
            <Button type="button" variant="outline" onClick={() => onOpen(installUrl, docsToastLabel)}>
              <Link2 size={14} aria-hidden="true" />
              {openDocsLabel}
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

export default memo(SettingsManualSetupCardComponent);
