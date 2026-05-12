import ToolsChoiceCard from "../../components/ToolsChoiceCard";
import ToolsToggleCard from "../../components/ToolsToggleCard";

type UiText = (zh: string, en: string, ja?: string) => string;

/* eslint-disable @typescript-eslint/no-explicit-any */
export interface CodexTabProps {
  uiText: UiText;
  codexApproval: string;
  codexApprovalOptions: any[];
  handleSelectCodexApproval: (value: string | number) => void;
  codexReasoning: string;
  codexReasoningOptions: any[];
  handleSelectCodexReasoning: (value: string | number) => void;
  codexDisableStorage: boolean;
  handleToggleCodexDisableStorage: any;
  codexContextWindow1M: boolean;
  handleToggleCodexContextWindow1M: any;
}

export default function CodexTab(props: CodexTabProps) {
  const {
    uiText,
    codexApproval,
    codexApprovalOptions,
    handleSelectCodexApproval,
    codexReasoning,
    codexReasoningOptions,
    handleSelectCodexReasoning,
    codexDisableStorage,
    handleToggleCodexDisableStorage,
    codexContextWindow1M,
    handleToggleCodexContextWindow1M,
  } = props;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      {/* Approval Mode */}
      <ToolsChoiceCard
        title={uiText("审批模式", "Approval Mode", "承認モード")}
        description={uiText("操作确认级别", "Action confirmation level", "操作確認レベル")}
        value={codexApproval}
        onSelect={handleSelectCodexApproval}
        options={codexApprovalOptions}
      />

      {/* Reasoning Effort */}
      <ToolsChoiceCard
        title={uiText("推理强度", "Reasoning Effort", "推論強度")}
        description={uiText("模型推理计算量", "Model reasoning compute", "モデルの推論計算量")}
        value={codexReasoning}
        onSelect={handleSelectCodexReasoning}
        options={codexReasoningOptions}
      />

      {/* Disable Response Storage */}
      <ToolsToggleCard
        title={uiText("禁用响应存储", "Disable Response Storage", "応答保存を無効化")}
        description={uiText(
          "不保存 API 响应到本地",
          "Don't save API responses locally",
          "API 応答をローカルに保存しません",
        )}
        value={codexDisableStorage}
        onChange={handleToggleCodexDisableStorage}
        labelOn={uiText("已禁用", "Disabled", "無効")}
        labelOff={uiText("已启用", "Enabled", "有効")}
      />

      {/* 1M Context Window */}
      <ToolsToggleCard
        title={uiText("1M 上下文窗口", "1M Context Window", "1M コンテキスト窗")}
        description={uiText(
          "启用 1M token 上下文 (gpt-5)",
          "Enable 1M token context (gpt-5)",
          "1M トークンコンテキストを有効化 (gpt-5)",
        )}
        value={codexContextWindow1M}
        onChange={handleToggleCodexContextWindow1M}
        labelOn={uiText("已启用", "Enabled", "有効")}
        labelOff={uiText("已关闭", "Disabled", "無効")}
      />
    </div>
  );
}
