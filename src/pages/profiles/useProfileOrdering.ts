import { useCallback, useRef, useState, type Dispatch, type SetStateAction } from "react";
import { useReorderConfigProfilesMutation } from "../../hooks/mutations/profile";
import { showToast } from "../../components/Toast";
import { profileMoveTarget, reorderedProfileIds, type ProfileMoveDirection } from "../../lib/profileOrdering";
import type { ConfigProfile } from "./helpers";

interface ProfileOrderingOptions {
  profiles: ConfigProfile[];
  orderedProfiles: ConfigProfile[];
  filterTool: string;
  enabled: boolean;
  setProfiles: Dispatch<SetStateAction<ConfigProfile[]>>;
  reload: () => Promise<void>;
  localeText: (zh: string, en: string, ja?: string) => string;
}

export function useProfileOrdering({
  profiles,
  orderedProfiles,
  filterTool,
  enabled,
  setProfiles,
  reload,
  localeText,
}: ProfileOrderingOptions) {
  const mutation = useReorderConfigProfilesMutation();
  const savingRef = useRef(false);
  const [orderBusy, setOrderBusy] = useState(false);
  const [orderAnnouncement, setOrderAnnouncement] = useState("");

  const reorderProfiles = useCallback(
    async (sourceId: string, targetId: string) => {
      if (!enabled || !filterTool || savingRef.current) return;
      const next = reorderedProfileIds(
        orderedProfiles.map((profile) => profile.id),
        sourceId,
        targetId,
      );
      if (!next || orderedProfiles.some((profile) => profile.tool_id !== filterTool)) return;
      savingRef.current = true;
      setOrderBusy(true);
      setOrderAnnouncement("");
      const before = new Map(
        profiles.filter((profile) => profile.tool_id === filterTool).map((profile) => [profile.id, profile.sort_order]),
      );
      const positions = new Map(next.map((id, index) => [id, index]));
      setProfiles((current) =>
        current.map((profile) =>
          profile.tool_id === filterTool && positions.has(profile.id)
            ? { ...profile, sort_order: positions.get(profile.id)! }
            : profile,
        ),
      );
      try {
        await mutation.mutateAsync({ toolId: filterTool, orderedIds: next });
        const name = orderedProfiles.find((profile) => profile.id === sourceId)!.name;
        const position = positions.get(sourceId)! + 1;
        setOrderAnnouncement(
          localeText(
            `已将“${name}”移至第 ${position} 位`,
            `Moved “${name}” to position ${position}`,
            `「${name}」を ${position} 番目に移動しました`,
          ),
        );
      } catch (error) {
        // Restore only this move's positions; preserve new profiles and unrelated edits.
        setProfiles((current) =>
          current.map((profile) =>
            profile.tool_id === filterTool && before.has(profile.id) && profile.sort_order === positions.get(profile.id)
              ? { ...profile, sort_order: before.get(profile.id)! }
              : profile,
          ),
        );
        showToast(
          "error",
          localeText(`排序未保存：${error}`, `Order was not saved: ${error}`, `並び順を保存できませんでした：${error}`),
        );
        await reload().catch((reloadError) => console.warn("Failed to refresh profiles after reorder", reloadError));
      } finally {
        savingRef.current = false;
        setOrderBusy(false);
      }
    },
    [enabled, filterTool, localeText, mutation, orderedProfiles, profiles, reload, setProfiles],
  );

  const handleMoveProfile = useCallback(
    (id: string, direction: ProfileMoveDirection) => {
      const target = profileMoveTarget(
        orderedProfiles.findIndex((profile) => profile.id === id),
        orderedProfiles.length,
        direction,
      );
      if (target !== null) void reorderProfiles(id, orderedProfiles[target].id);
    },
    [orderedProfiles, reorderProfiles],
  );

  return { reorderProfiles, handleMoveProfile, orderBusy, orderAnnouncement };
}
