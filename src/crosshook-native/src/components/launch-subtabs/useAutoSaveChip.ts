import { useEffect, useRef, useState } from 'react';
import type { LaunchAutoSaveStatus } from '../../types';
import type { LaunchOptimizationsPanelStatus } from '../LaunchOptimizationsPanel';

const TONE_PRIORITY: Record<string, number> = { idle: 0, success: 1, warning: 2, saving: 3, error: 4 };

interface UseAutoSaveChipOptions {
  launchOptimizationsStatus?: LaunchOptimizationsPanelStatus;
  gamescopeAutoSaveStatus?: LaunchAutoSaveStatus;
  mangoHudAutoSaveStatus?: LaunchAutoSaveStatus;
}

interface UseAutoSaveChipResult {
  combinedAutoSaveStatus: LaunchAutoSaveStatus;
  chipVisible: boolean;
}

export function useAutoSaveChip({
  launchOptimizationsStatus,
  gamescopeAutoSaveStatus,
  mangoHudAutoSaveStatus,
}: UseAutoSaveChipOptions): UseAutoSaveChipResult {
  const allStatuses: LaunchAutoSaveStatus[] = [
    launchOptimizationsStatus ?? { tone: 'idle', label: '' },
    gamescopeAutoSaveStatus ?? { tone: 'idle', label: '' },
    mangoHudAutoSaveStatus ?? { tone: 'idle', label: '' },
  ];
  const combinedAutoSaveStatus = allStatuses.reduce<LaunchAutoSaveStatus>(
    (best, s) => ((TONE_PRIORITY[s.tone] ?? 0) > (TONE_PRIORITY[best.tone] ?? 0) ? s : best),
    { tone: 'idle', label: '' }
  );

  // Fade chip out 3s after success
  const [chipVisible, setChipVisible] = useState(false);
  const chipTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    if (chipTimerRef.current !== null) {
      clearTimeout(chipTimerRef.current);
      chipTimerRef.current = null;
    }
    if (combinedAutoSaveStatus.tone !== 'idle') {
      setChipVisible(true);
      if (combinedAutoSaveStatus.tone === 'success') {
        chipTimerRef.current = setTimeout(() => setChipVisible(false), 3000);
      }
    } else {
      setChipVisible(false);
    }
    return () => {
      if (chipTimerRef.current !== null) clearTimeout(chipTimerRef.current);
    };
  }, [combinedAutoSaveStatus.tone]);

  return { combinedAutoSaveStatus, chipVisible };
}
