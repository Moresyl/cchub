import type { DraftModelCheckResult } from "./draftModelChecks";

type Text = (zh: string, en: string, ja?: string) => string;

export function modelCheckMessage(result: DraftModelCheckResult, text: Text): string {
  if (result.status === "healthy") return text("模型响应验证通过", "Model reply verified", "モデル応答の検証に成功");
  switch (result.httpStatus) {
    case 401:
      return text(
        "认证失败，请检查当前草稿的密钥或登录状态",
        "Authentication failed. Check the draft key or sign-in status.",
        "認証に失敗しました。下書きのキーまたはログイン状態を確認してください。",
      );
    case 403:
      return text(
        "当前账号或密钥没有访问权限",
        "This account or key does not have access.",
        "このアカウントまたはキーにはアクセス権限がありません。",
      );
    case 404:
      return text(
        "模型或接口地址不存在",
        "The model or endpoint was not found.",
        "モデルまたはエンドポイントが見つかりません。",
      );
    case 429:
      return text(
        "请求受限或额度不足，请稍后重试",
        "Rate limit or insufficient quota. Try again later.",
        "リクエスト制限またはクォータ不足です。後でもう一度お試しください。",
      );
  }
  if (result.httpStatus !== null && result.httpStatus >= 500)
    return text(
      "供应商服务暂时出现错误",
      "The provider service returned a server error.",
      "プロバイダーのサービスでエラーが発生しました。",
    );
  if (result.message.includes("timed out"))
    return text("模型请求超时", "The model request timed out.", "モデルリクエストがタイムアウトしました。");
  return result.message;
}
