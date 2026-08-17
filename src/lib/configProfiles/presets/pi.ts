import type { ConfigPreset } from "../types";

export const piPresets: ConfigPreset[] = [
  {
    id: "pi-custom",
    toolId: "pi",
    name: "Pi Custom",
    baseUrl: "",
    model: "",
    apiProtocol: "openai-completions",
    category: "custom",
    featured: true,
  },
];
