import { memo, useEffect, useState, type ComponentType } from "react";

interface ProviderIconProps {
  iconUrl?: string;
  fallbackIcon: ComponentType<{ size?: number }>;
  size?: number;
}

function isRemoteIconUrl(value: string) {
  return /^https?:\/\//i.test(value.trim());
}

function ProviderIconComponent({
  iconUrl,
  fallbackIcon: FallbackIcon,
  size = 16,
}: ProviderIconProps) {
  const normalizedUrl = iconUrl?.trim() || "";
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setFailed(false);
  }, [normalizedUrl]);

  if (normalizedUrl && !failed && isRemoteIconUrl(normalizedUrl)) {
    return (
      <img
        src={normalizedUrl}
        alt=""
        width={size}
        height={size}
        onError={() => setFailed(true)}
        style={{
          width: size,
          height: size,
          objectFit: "cover",
          borderRadius: 6,
          display: "block",
        }}
      />
    );
  }

  return <FallbackIcon size={size} />;
}

export default memo(ProviderIconComponent);
