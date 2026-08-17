import { Boxes, Database, Info, Settings2, Stethoscope } from "lucide-react";

export type SettingsCategory = "general" | "integrations" | "data" | "diagnostics" | "about";

interface SettingsCategoryNavProps {
  active: SettingsCategory;
  locale: string;
  onChange: (category: SettingsCategory) => void;
}

export default function SettingsCategoryNav({ active, locale, onChange }: SettingsCategoryNavProps) {
  const zh = locale === "zh";
  const items = [
    { id: "general", icon: Settings2, label: zh ? "常用" : "General" },
    { id: "integrations", icon: Boxes, label: zh ? "工具与集成" : "Integrations" },
    { id: "data", icon: Database, label: zh ? "数据与备份" : "Data & Backup" },
    { id: "diagnostics", icon: Stethoscope, label: zh ? "网络与诊断" : "Diagnostics" },
    { id: "about", icon: Info, label: zh ? "关于" : "About" },
  ] as const;

  return (
    <nav
      aria-label={zh ? "设置分类" : "Settings categories"}
      className="tab-bar"
      style={{ marginBottom: 20, overflowX: "auto", position: "sticky", top: 0, zIndex: 4 }}
    >
      {items.map((item) => {
        const Icon = item.icon;
        return (
          <button
            key={item.id}
            className={`tab-item ${active === item.id ? "active" : ""}`}
            type="button"
            aria-pressed={active === item.id}
            onClick={() => onChange(item.id)}
            style={{ display: "inline-flex", alignItems: "center", gap: 6, whiteSpace: "nowrap" }}
          >
            <Icon size={14} />
            {item.label}
          </button>
        );
      })}
    </nav>
  );
}
