import { describe, expect, it } from "vitest";
import { localizedReleaseNotes } from "./releaseNotes";

describe("localized release notes", () => {
  it("selects explicit translations and falls back to English without inventing a translation", () => {
    const body = "# Features\n\nEnglish\n\n<!-- lang:zh -->\n\n# 更新\n\n中文";
    expect(localizedReleaseNotes(body, "en")).toBe("# Features\n\nEnglish");
    expect(localizedReleaseNotes(body, "zh")).toBe("# 更新\n\n中文");
    expect(localizedReleaseNotes(body, "ja")).toBe("# Features\n\nEnglish");
  });
  it("supports Chinese-first and Japanese translations without depending on marker order", () => {
    const body = "<!-- lang:zh -->\n中文\n<!-- lang:ja -->\n日本語\n<!-- lang:en -->\nEnglish";
    expect(localizedReleaseNotes(body, "ja")).toBe("日本語");
    expect(localizedReleaseNotes(body, "en")).toBe("English");
    expect(localizedReleaseNotes(body, "zh")).toBe("中文");
  });
  it("keeps the common download section for historical bilingual releases", () => {
    const body =
      "## 更新内容 / Highlights\n\n### 新增 / 更新\n\n- 中文\n\n## English Summary\n\n- English\n\n## 下载 / Download\n\n| File | Platform |\n|---|---|\n| app.exe | Windows |";
    const common = body.slice(body.indexOf("## 下载"));
    expect(localizedReleaseNotes(body, "zh")).toBe(
      "## 更新内容 / Highlights\n\n### 新增 / 更新\n\n- 中文\n\n" + common,
    );
    expect(localizedReleaseNotes(body, "en")).toBe("- English\n\n" + common);
    expect(localizedReleaseNotes(body, "ja")).toBe("- English\n\n" + common);
  });
  it.each([null, undefined, "", "   "])("handles empty release notes %s", (body) => {
    expect(localizedReleaseNotes(body, "zh")).toBe("");
  });
  it("preserves old monolingual releases and ignores metadata inside fenced code", () => {
    const body = "# Existing notes\n\n```md\n<!-- lang:zh -->\n## English Summary\n```\n\nMore details.";
    expect(localizedReleaseNotes(body, "zh")).toBe(body);
    const tilde = "~~~~md\n<!-- lang:en -->\n~~~\n<!-- lang:ja -->\n~~~~\n<!-- lang:zh -->\n中文";
    expect(localizedReleaseNotes(tilde, "en")).toBe("~~~~md\n<!-- lang:en -->\n~~~\n<!-- lang:ja -->\n~~~~");
    expect(localizedReleaseNotes(tilde, "zh")).toBe("中文");
  });
  it("falls back to available content when the requested translation is empty", () => {
    expect(localizedReleaseNotes("English\n<!-- lang:zh -->\n  ", "zh")).toBe("English");
    expect(localizedReleaseNotes("<!-- lang:zh -->\n中文", "en")).toBe("中文");
    expect(localizedReleaseNotes("中文\n## English Summary\n\n", "en")).toBe("中文");
  });
  it("retains CRLF, links and code within the selected section", () => {
    const content = "### English\r\n\r\n[Guide](https://example.test)\r\n\r\n```sh\r\necho hello\r\n```";
    expect(localizedReleaseNotes(content + "\r\n<!-- lang:zh -->\r\n中文", "en")).toBe(content);
  });
});
