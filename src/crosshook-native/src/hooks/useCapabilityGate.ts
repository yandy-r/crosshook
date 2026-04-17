import { useCallback, useMemo } from 'react';
import { useHostReadinessContext } from '../context/HostReadinessContext';
import type { CapabilityState, HostToolCheckResult, HostToolInstallCommand } from '../types/onboarding';
import { getFirstCapabilityDocsUrl } from '../utils/capabilityDocs';
import { copyToClipboard } from '../utils/clipboard';

const EMPTY_MISSING_REQUIRED: HostToolCheckResult[] = [];

export interface CapabilityGate {
  state: CapabilityState;
  rationale: string | null;
  missingRequired: HostToolCheckResult[];
  /** Tool ids missing for this capability (required ∪ optional). */
  missingToolIds: string[];
  installHint: HostToolInstallCommand | null;
  onCopyCommand?: () => Promise<void>;
  docsUrl: string | null;
}

export function useCapabilityGate(capabilityId: string): CapabilityGate {
  const { capabilities } = useHostReadinessContext();

  const capability = useMemo(
    () => capabilities.find((entry) => entry.id === capabilityId),
    [capabilities, capabilityId]
  );

  const installHint = capability?.install_hints[0] ?? null;
  const canCopyCommand = (installHint?.command ?? '').trim() !== '';

  const missingToolIds = useMemo(() => {
    if (capability == null) {
      return [];
    }
    return [...capability.missing_required, ...capability.missing_optional].map((t) => t.tool_id);
  }, [capability]);

  const onCopyCommand = useCallback(async () => {
    if (!canCopyCommand || installHint == null) {
      return;
    }
    await copyToClipboard(installHint.command);
  }, [canCopyCommand, installHint]);

  return useMemo(
    () => ({
      state: capability?.state ?? 'unavailable',
      rationale: capability?.rationale ?? null,
      missingRequired: capability?.missing_required ?? EMPTY_MISSING_REQUIRED,
      missingToolIds,
      installHint,
      onCopyCommand: canCopyCommand ? onCopyCommand : undefined,
      docsUrl: getFirstCapabilityDocsUrl(capability),
    }),
    [capability, canCopyCommand, installHint, missingToolIds, onCopyCommand]
  );
}

export default useCapabilityGate;
