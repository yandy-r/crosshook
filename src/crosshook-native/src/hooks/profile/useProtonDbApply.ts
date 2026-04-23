import { useCallback, useEffect, useRef, useState } from 'react';
import type { GameProfile } from '../../types/profile';
import type { AcceptSuggestionRequest, ProtonDbRecommendationGroup } from '../../types/protondb';
import type { OptimizationCatalogPayload } from '../../utils/optimization-catalog';
import {
  applyProtonDbGroupToProfile,
  mergeProtonDbEnvVarGroup,
  type PendingProtonDbOverwrite,
} from '../../utils/protondb';

export interface UseProtonDbApplyInput {
  profile: GameProfile;
  catalog: OptimizationCatalogPayload | null | undefined;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  onAcceptSuggestion?: (request: AcceptSuggestionRequest) => Promise<void>;
}

export interface UseProtonDbApplyReturn {
  pendingOverwrite: PendingProtonDbOverwrite | null;
  applyingGroupId: string | null;
  statusMessage: string | null;
  applyGroup: (group: ProtonDbRecommendationGroup, overwriteKeys: readonly string[]) => void;
  applyEnvVars: (group: ProtonDbRecommendationGroup) => void;
  acceptSuggestion: (request: AcceptSuggestionRequest) => Promise<void>;
  clearOverwrite: () => void;
  updateOverwriteResolution: (next: PendingProtonDbOverwrite | null) => void;
  resetAll: () => void;
}

const STATUS_AUTO_CLEAR_MS = 4000;

export function useProtonDbApply({
  profile,
  catalog,
  onUpdateProfile,
  onAcceptSuggestion,
}: UseProtonDbApplyInput): UseProtonDbApplyReturn {
  const [pendingOverwrite, setPendingOverwrite] = useState<PendingProtonDbOverwrite | null>(null);
  const [applyingGroupId, setApplyingGroupId] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const statusClearTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Auto-clear statusMessage after 4 seconds
  useEffect(() => {
    if (statusMessage == null) {
      return;
    }
    if (statusClearTimerRef.current != null) {
      clearTimeout(statusClearTimerRef.current);
    }
    statusClearTimerRef.current = setTimeout(() => {
      setStatusMessage(null);
      statusClearTimerRef.current = null;
    }, STATUS_AUTO_CLEAR_MS);

    return () => {
      if (statusClearTimerRef.current != null) {
        clearTimeout(statusClearTimerRef.current);
        statusClearTimerRef.current = null;
      }
    };
  }, [statusMessage]);

  const applyGroup = useCallback(
    (group: ProtonDbRecommendationGroup, overwriteKeys: readonly string[]) => {
      const result = {
        appliedKeys: [] as string[],
        unchangedKeys: [] as string[],
        toggledOptionIds: [] as string[],
      };
      onUpdateProfile((current) => {
        const applyResult = applyProtonDbGroupToProfile(current, group, overwriteKeys, catalog ?? null);
        result.appliedKeys = applyResult.appliedKeys;
        result.unchangedKeys = applyResult.unchangedKeys;
        result.toggledOptionIds = applyResult.toggledOptionIds;
        return applyResult.nextProfile;
      });
      setApplyingGroupId(null);
      setPendingOverwrite(null);

      const appliedCount = result.appliedKeys.length;
      const unchangedCount = result.unchangedKeys.length;
      const toggledCount = result.toggledOptionIds.length;
      if (appliedCount > 0 || toggledCount > 0) {
        const parts: string[] = [];
        if (toggledCount > 0) parts.push(`${toggledCount} optimization${toggledCount === 1 ? '' : 's'}`);
        if (appliedCount - toggledCount > 0) {
          const envCount = appliedCount - toggledCount;
          parts.push(`${envCount} env var${envCount === 1 ? '' : 's'}`);
        }
        setStatusMessage(
          `Applied ${parts.join(' and ')}${
            unchangedCount > 0
              ? ` and left ${unchangedCount} existing match${unchangedCount === 1 ? '' : 'es'} unchanged`
              : ''
          }.`
        );
        return;
      }

      if (unchangedCount > 0) {
        setStatusMessage('All suggested ProtonDB environment variables already match the current profile.');
        return;
      }

      setStatusMessage('No ProtonDB environment-variable changes were applied.');
    },
    [onUpdateProfile, catalog]
  );

  const applyEnvVars = useCallback(
    (group: ProtonDbRecommendationGroup) => {
      const envVars = group.env_vars ?? [];
      if (envVars.length === 0) {
        return;
      }

      setApplyingGroupId(group.group_id?.trim() || group.title?.trim() || null);
      const merge = mergeProtonDbEnvVarGroup(profile.launch.custom_env_vars, group);
      if (merge.conflicts.length === 0) {
        applyGroup(group, []);
        return;
      }

      setApplyingGroupId(null);
      setPendingOverwrite({
        group,
        conflicts: merge.conflicts,
        resolutions: Object.fromEntries(merge.conflicts.map((conflict) => [conflict.key, 'keep_current' as const])),
      });
      setStatusMessage(null);
    },
    [applyGroup, profile.launch.custom_env_vars]
  );

  const acceptSuggestion = useCallback(
    async (request: AcceptSuggestionRequest): Promise<void> => {
      if (onAcceptSuggestion) {
        await onAcceptSuggestion(request);
      }
    },
    [onAcceptSuggestion]
  );

  const clearOverwrite = useCallback(() => {
    setPendingOverwrite(null);
  }, []);

  const updateOverwriteResolution = useCallback((next: PendingProtonDbOverwrite | null) => {
    setPendingOverwrite(next);
  }, []);

  const resetAll = useCallback(() => {
    setPendingOverwrite(null);
    setApplyingGroupId(null);
    setStatusMessage(null);
  }, []);

  return {
    pendingOverwrite,
    applyingGroupId,
    statusMessage,
    applyGroup,
    applyEnvVars,
    acceptSuggestion,
    clearOverwrite,
    updateOverwriteResolution,
    resetAll,
  };
}
