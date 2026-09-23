import LoadingState from "../../components/states/LoadingState";
import ErrorState from "../../components/states/ErrorState";

interface ProfilesLoadStateProps {
  error: string | null;
  localeText: (zh: string, en: string, ja?: string) => string;
  onRetry: () => void;
}

export default function ProfilesLoadState({ error, localeText, onRetry }: ProfilesLoadStateProps) {
  if (!error) return <LoadingState label={localeText("加载中...", "Loading...", "読み込み中...")} />;

  return (
    <ErrorState
      title={localeText("配置加载失败", "Failed to load profiles", "設定の読み込みに失敗しました")}
      message={localeText(
        "暂时无法读取本机配置。请确认应用服务正在运行，然后重试。",
        "Local configurations are temporarily unavailable. Check that the app service is running, then retry.",
        "ローカル設定を読み込めません。アプリサービスの状態を確認して、もう一度お試しください。",
      )}
      retryLabel={localeText("刷新", "Refresh", "再読み込み")}
      onRetry={onRetry}
    />
  );
}
