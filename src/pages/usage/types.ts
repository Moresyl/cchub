export interface UsageAnalytics {
  days: number;
  start_date: string;
  end_date: string;
  summary: {
    total_requests: number;
    success_requests: number;
    success_rate: number;
    input_tokens: number;
    output_tokens: number;
    total_tokens?: number;
    cache_read_tokens: number;
    cache_creation_tokens: number;
    total_cost_usd: string;
  };
  trends: {
    date: string;
    requests: number;
    success_requests: number;
    input_tokens: number;
    output_tokens: number;
    total_cost_usd: string;
  }[];
  providers: {
    provider_name: string;
    app_id: string;
    requests: number;
    success_rate: number;
    total_tokens: number;
    total_cost_usd: string;
    avg_latency_ms: number;
  }[];
  models: {
    model: string;
    requests: number;
    success_rate: number;
    total_tokens: number;
    total_cost_usd: string;
    avg_latency_ms: number;
  }[];
}

export interface UsageFilters {
  days: number;
  appId: string;
  providerName: string;
  model: string;
}
