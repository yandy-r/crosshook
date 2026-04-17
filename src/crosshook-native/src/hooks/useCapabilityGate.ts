import { useCallback, useMemo } from 'react';
import type { Capability, CapabilityState, HostToolCheckResult, HostToolInstallCommand } from '../types/onboarding';
import { copyToClipboard } from '../utils/clipboard';
import { useHostReadiness } from './useHostReadiness';

const EMPTY_MISSING_REQUIRED: HostToolCheckResult[] = [];

export interface CapabilityGate {
  state: CapabilityState;
  rationale: string | null;
  missingRequired: HostToolCheckResult[];
  installHint: HostToolInstallCommand | null;
  onDismiss?: () => void;
  onCopyCommand?: () => Promise<void>;
  docsUrl: string | null;
}

function firstDocsUrl(capability: Capability | undefined): string | null {
  if (capability == null) {
    return null;
  }

  const missingTools = [...capability.missing_required, ...capability.missing_optional];
  const docsUrl = missingTools.find((tool) => (tool.docs_url ?? '').trim() !== '')?.docs_url?.trim();
  return docsUrl && docsUrl.length > 0 ? docsUrl : null;
}

export function useCapabilityGate(capabilityId: string): CapabilityGate {
  const { capabilities } = useHostReadiness();

  const capability = useMemo(
    () => capabilities.find((entry) => entry.id === capabilityId),
    [capabilities, capabilityId]
  );

  const installHint = capability?.install_hints[0] ?? null;
  const canCopyCommand = (installHint?.command ?? '').trim() !== '';
  const canDismiss = installHint !== null || (capability?.missing_required.length ?? 0) > 0;

  const onCopyCommand = useCallback(async () => {
    if (!canCopyCommand || installHint == null) {
      return;
    }
    await copyToClipboard(installHint.command);
  }, [canCopyCommand, installHint]);

  const onDismiss = useCallback(() => {
    // Placeholder for the per-tool nag dismissal wiring that lands with the
    // host readiness dashboard consumers.
  }, []);

  return useMemo(
    () => ({
      state: capability?.state ?? 'unavailable',
      rationale: capability?.rationale ?? null,
      missingRequired: capability?.missing_required ?? EMPTY_MISSING_REQUIRED,
      installHint,
      onDismiss: canDismiss ? onDismiss : undefined,
      onCopyCommand: canCopyCommand ? onCopyCommand : undefined,
      docsUrl: firstDocsUrl(capability),
    }),
    [capability, canCopyCommand, canDismiss, installHint, onCopyCommand, onDismiss]
  );
}

export default useCapabilityGate;
