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
      message={error}
      retryLabel={localeText("刷新", "Refresh", "再読み込み")}
      onRetry={onRetry}
    />
  );
}
