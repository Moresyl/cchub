export interface MappingRule {
  from: string;
  to: string;
  matchMode: "contains" | "exact";
}

export interface OptimizerConfig {
  enabled: boolean;
  thinkingOptimizer: boolean;
  cacheInjection: boolean;
  cacheTtl: string;
  bodyFilter: boolean;
  bodyFilterWhitelist: string[];
  modelMapper: boolean;
  modelMapperDefault: string;
  modelMapperRules: MappingRule[];
  copilotOptimizer: boolean;
  copilotMergeToolResults: boolean;
  copilotSanitizeOrphans: boolean;
  copilotStripThinking: boolean;
  copilotCompactDetection: boolean;
  copilotSubagentDetection: boolean;
  copilotModelNormalization: boolean;
  codexFieldStripping: boolean;
  circuitFailureThreshold: number;
  circuitSuccessThreshold: number;
  circuitTimeoutSecs: number;
  failoverEnabled: boolean;
  maxProfileRetries: number;
  streamingFirstByteTimeout: number;
  streamingIdleTimeout: number;
  nonStreamingTimeout: number;
}

export interface RectifierConfig {
  enabled: boolean;
  thinkingSignature: boolean;
  thinkingBudget: boolean;
}

export interface ProxyAdvancedSettings {
  config: OptimizerConfig;
  rectifierConfig: RectifierConfig;
  revision: string;
}
