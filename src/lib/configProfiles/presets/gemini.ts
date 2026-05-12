/* eslint-disable @typescript-eslint/no-explicit-any */
import type { ConfigPreset } from "../types";

export const geminiPresets: ConfigPreset[] = [
  // === 官方 ===
  {
    id: "gemini-official",
    toolId: "gemini",
    name: "Google Official",
    websiteUrl: "https://ai.google.dev/",
    apiKeyUrl: "https://aistudio.google.com/apikey",
    category: "official",
    badge: "OAuth",
    featured: true,
    baseUrl: "",
    model: "",
    requiresOAuth: true,
    providerType: "google_oauth",
  },
  // === 聚合 ===
  {
    id: "gemini-openrouter",
    toolId: "gemini",
    name: "OpenRouter",
    websiteUrl: "https://openrouter.ai",
    apiKeyUrl: "https://openrouter.ai/keys",
    category: "aggregator",
    badge: "聚合",
    baseUrl: "https://openrouter.ai/api",
    model: "gemini-3.1-pro",
  },
  // === 自定义 ===
  { id: "gemini-custom", toolId: "gemini", name: "自定义", category: "custom", baseUrl: "", model: "gemini-3.1-pro" },
];
