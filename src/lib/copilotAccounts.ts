import type { ModelBilling } from "./modelBilling";

export interface GitHubAccount {
  id: string;
  login: string;
  avatar_url: string | null;
  authenticated_at: number;
  revision: string;
}

export interface CopilotAuthStatus {
  accounts: GitHubAccount[];
  default_account_id: string | null;
  authenticated: boolean;
  username: string | null;
  expires_at: number | null;
}

export interface CopilotQuota {
  entitlement: number | null;
  remaining: number | null;
  percent_remaining: number | null;
  unlimited: boolean;
}

export interface CopilotUsage {
  copilot_plan: string;
  quota_reset_date: string;
  quota_snapshots: {
    chat: CopilotQuota | null;
    completions: CopilotQuota | null;
    premium_interactions: CopilotQuota | null;
  };
}

export interface CopilotModel {
  id: string;
  name: string;
  vendor: string;
  billing?: ModelBilling;
}

export type CopilotResourceFailure =
  | "sign_in_required"
  | "subscription_unavailable"
  | "rate_limited"
  | "unavailable"
  | "invalid_response"
  | "timeout";

export interface CopilotAccountResources {
  account: GitHubAccount;
  fetched_at: string;
  usage: CopilotUsage | null;
  models: CopilotModel[] | null;
  usage_error: CopilotResourceFailure | null;
  models_error: CopilotResourceFailure | null;
}
