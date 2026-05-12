/* eslint-disable @typescript-eslint/no-explicit-any */
import { invoke } from "@tauri-apps/api/core";

import { showToast } from "../../components/Toast";
import { type FeaturedSkillBundle } from "../../components/FeaturedSkillBundleCard";
import type { DetectedTool } from "../../types/skills";

import type { SkillEntry } from "./helpers";

export function buildFeaturedBundles(locale: string): FeaturedSkillBundle[] {
  return [
    {
      id: "obra/superpowers",
      title: "Superpowers",
      owner: "obra",
      repo: "superpowers",
      branch: "main",
      totalSkills: 14,
      starsLabel: "154k★",
      description:
        locale === "zh"
          ? "Jesse Vincent 的 agentic 开发方法论：TDD、代码评审、并行 agents、系统化调试 —— 一次安装 14 个协同技能"
          : locale === "ja"
            ? "Jesse Vincent 氏のエージェント開発方法論：TDD・コードレビュー・並列エージェント・体系的デバッグ — 14 件の連携スキルを一括インストール"
            : "Jesse Vincent's agentic delivery framework: TDD, code review, parallel agents, systematic debugging — install 14 coordinated skills at once",
      tags: [
        "brainstorming",
        "writing-plans",
        "test-driven-development",
        "subagent-driven-development",
        "systematic-debugging",
        "using-git-worktrees",
        "requesting-code-review",
        "receiving-code-review",
        "executing-plans",
        "verification-before-completion",
      ],
      githubUrl: "https://github.com/obra/superpowers",
    },
  ];
}

export function computeBundleInstalledCount(
  bundle: FeaturedSkillBundle,
  skillEntries: SkillEntry[],
  currentToolInstalledSkills: Set<string>,
  bundleProgress: Record<string, number>,
): number {
  // 优先用从市场加载到的 skillEntries 精确比对 bundle 下每一项的安装态。
  // 这条路径仅在用户当次会话已点过"一键安装"或主动加载该 source 后才命中。
  const prefix = `${bundle.owner}/${bundle.repo}:`;
  const bundleEntries = skillEntries.filter((s) => s.id.startsWith(prefix));
  if (bundleEntries.length > 0) {
    return bundleEntries.filter((s) => currentToolInstalledSkills.has(s.name.toLowerCase())).length;
  }

  // 重启 CCHub 后 skillEntries 不会自动重新拉取（远程 fetch 是用户驱动的），
  // 但 currentToolInstalledSkills 来自本地 scan_skills，包含磁盘上所有 skill。
  // 这里用 bundle.tags（其元素就是 superpowers 的 skill 文件名）做本地交集，
  // 让"是否完整安装"的判断不再依赖远程列表是否已加载。
  if (bundle.tags.length > 0) {
    const installedKnown = bundle.tags.reduce(
      (count, tag) => (currentToolInstalledSkills.has(tag.toLowerCase()) ? count + 1 : count),
      0,
    );
    if (installedKnown === bundle.tags.length) {
      return bundle.totalSkills;
    }
    return Math.max(installedKnown, bundleProgress[bundle.id] ?? 0);
  }

  return bundleProgress[bundle.id] ?? 0;
}

export interface InstallBundleContext {
  bundle: FeaturedSkillBundle;
  locale: string;
  activeTool: string;
  tools: DetectedTool[];
  setInstallingBundle: (v: string | null) => void;
  setBundleProgress: (updater: (prev: Record<string, number>) => Record<string, number>) => void;
  setSkillEntries: (updater: (prev: SkillEntry[]) => SkillEntry[]) => void;
  setCustomSources: (
    updater: (
      prev: { url: string; count: number; skillIds: string[] }[],
    ) => { url: string; count: number; skillIds: string[] }[],
  ) => void;
  setInstalledSkillsByTool: (updater: (prev: Record<string, Set<string>>) => Record<string, Set<string>>) => void;
}

export async function performInstallBundle(ctx: InstallBundleContext): Promise<void> {
  const { bundle, locale, activeTool, tools } = ctx;
  ctx.setInstallingBundle(bundle.id);
  ctx.setBundleProgress((prev) => ({ ...prev, [bundle.id]: 0 }));
  try {
    const skills = await invoke<SkillEntry[]>("fetch_skills_from_repo", {
      owner: bundle.owner,
      repo: bundle.repo,
      branch: bundle.branch,
    });

    ctx.setSkillEntries((prev) => {
      const existingIds = new Set(prev.map((s) => s.id));
      const newEntries = skills.filter((s) => !existingIds.has(s.id));
      return [...prev, ...newEntries];
    });

    const sourceUrl = `github:${bundle.owner}/${bundle.repo}`;
    ctx.setCustomSources((prev) => {
      if (prev.some((s) => s.url === sourceUrl)) return prev;
      return [...prev, { url: sourceUrl, count: skills.length, skillIds: skills.map((s) => s.id) }];
    });

    const tool = tools.find((t) => t.id === activeTool);
    let installedCount = 0;
    const newlyInstalledNames: string[] = [];
    for (const skill of skills) {
      try {
        await invoke<string>("install_skill_from_marketplace", {
          name: skill.id,
          content: skill.content,
          description: skill.description,
          triggerCommand: null,
          sourceUrl: skill.github_url,
          targetDir: tool?.skills_dir ?? null,
        });
        newlyInstalledNames.push(skill.name.toLowerCase());
        installedCount += 1;
        ctx.setBundleProgress((prev) => ({ ...prev, [bundle.id]: installedCount }));
      } catch (err) {
        console.error(`Failed to install ${skill.id}`, err);
      }
    }

    if (newlyInstalledNames.length > 0) {
      ctx.setInstalledSkillsByTool((prev) => {
        const next = { ...prev };
        const set = new Set(next[activeTool] ?? []);
        newlyInstalledNames.forEach((n) => set.add(n));
        next[activeTool] = set;
        return next;
      });
    }

    const failedCount = skills.length - newlyInstalledNames.length;
    if (failedCount === 0) {
      showToast(
        "success",
        locale === "zh"
          ? `${bundle.title}：${skills.length} 个技能全部安装完成`
          : `${bundle.title}: installed ${skills.length} skills`,
      );
    } else {
      showToast(
        "error",
        locale === "zh"
          ? `${bundle.title}：${newlyInstalledNames.length} / ${skills.length} 安装成功，${failedCount} 个失败`
          : `${bundle.title}: ${newlyInstalledNames.length}/${skills.length} installed, ${failedCount} failed`,
      );
    }
  } catch (e) {
    console.error(e);
    showToast("error", locale === "zh" ? `${bundle.title} 安装失败: ${e}` : `${bundle.title} install failed: ${e}`);
  } finally {
    ctx.setInstallingBundle(null);
  }
}

export function buildRecommendedRepos(locale: string): { name: string; branch: string; desc: string }[] {
  return [
    {
      name: "levnikolaevich/claude-code-skills",
      branch: "master",
      desc: locale === "zh" ? "125 个技能 — 全流程交付（研发/测试/评审）" : "125 skills — full delivery workflow",
    },
    {
      name: "rohitg00/awesome-claude-code-toolkit",
      branch: "main",
      desc: locale === "zh" ? "35 个技能 + 135 个 Agent + 42 个命令" : "35 skills + 135 agents + 42 commands",
    },
    {
      name: "JimLiu/baoyu-skills",
      branch: "main",
      desc: locale === "zh" ? "18 个技能 — 宝玉技能合集" : "18 skills — Baoyu skills collection",
    },
    {
      name: "cexll/myclaude",
      branch: "master",
      desc: locale === "zh" ? "11 个技能 — 浏览器/代码/开发等" : "11 skills — browser/code/dev",
    },
  ];
}
