import { memo } from "react";
import type { DeepLinkImportRequest } from "../lib/deeplink";

interface SkillPreviewSectionProps {
  current: DeepLinkImportRequest;
  fetchDescription: string;
}

function SkillPreviewSectionComponent({ current, fetchDescription }: SkillPreviewSectionProps) {
  return (
    <section className="section-card" style={{ padding: 14 }}>
      <div className="field-label">Skill</div>
      <div style={{ display: "grid", gap: 10 }}>
        {current.name && <span className="badge badge-muted">{current.name}</span>}
        {current.repo && (
          <div>
            <div className="field-label">Repository</div>
            <div style={{ fontSize: 13, fontFamily: "'JetBrains Mono', monospace" }}>{current.repo}</div>
          </div>
        )}
        {(current.branch || current.directory) && (
          <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
            {current.branch && <span className="badge badge-muted">{`Branch: ${current.branch}`}</span>}
            {current.directory && <span className="badge badge-muted">{`Dir: ${current.directory}`}</span>}
          </div>
        )}
        <div style={{ fontSize: 13, color: "var(--text-secondary)" }}>{fetchDescription}</div>
      </div>
    </section>
  );
}

export const SkillPreviewSection = memo(SkillPreviewSectionComponent);

export function requestFingerprint(request: DeepLinkImportRequest) {
  return JSON.stringify(request);
}

export function normalizeDirectory(value: string | undefined) {
  return (value || "").replace(/\\/g, "/").replace(/^\/+|\/+$/g, "");
}
