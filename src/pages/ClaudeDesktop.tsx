import ClaudeDesktopProviders from "../components/ClaudeDesktopProviders";
import SettingsClaudeDesktopSection from "../components/SettingsClaudeDesktopSection";
import { getLocale } from "../lib/i18n";

export default function ClaudeDesktop() {
  const locale = getLocale();
  return (
    <div className="page-enter space-y-6">
      <div className="page-header">
        <h2 className="page-title">Claude Desktop</h2>
      </div>
      <ClaudeDesktopProviders locale={locale} />
      <SettingsClaudeDesktopSection locale={locale} />
    </div>
  );
}
