import { useEffect, useRef, useState } from 'react';
import { open as openUrl } from '@/lib/plugin-stubs/shell';
import type { Capability, CapabilityState } from '../../types/onboarding';
import { getFirstCapabilityDocsUrl } from '../../utils/capabilityDocs';
import { copyToClipboard } from '../../utils/clipboard';

export interface CapabilityTileProps {
  capability: Capability;
}

const STATE_LABEL: Record<CapabilityState, string> = {
  available: 'Available',
  degraded: 'Degraded',
  unavailable: 'Unavailable',
};

function formatCount(missing: number, total: number, kind: 'required' | 'optional'): string {
  if (total === 0) {
    return `No ${kind} tools defined`;
  }

  const ready = total - missing;

  if (missing === 0) {
    return `${total} ${kind} ${total === 1 ? 'tool' : 'tools'} ready`;
  }

  return `${ready} of ${total} ${kind} ${total === 1 ? 'tool' : 'tools'} ready`;
}

export function CapabilityTile({ capability }: CapabilityTileProps) {
  const requiredTotal = capability.required_tool_ids.length;
  const requiredMissing = capability.missing_required.length;
  const optionalTotal = capability.optional_tool_ids.length;
  const optionalMissing = capability.missing_optional.length;
  const docsUrl = getFirstCapabilityDocsUrl(capability);
  const installHint = capability.install_hints[0];
  const [copyStatus, setCopyStatus] = useState<'idle' | 'copied' | 'failed'>('idle');
  const copyResetTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (copyResetTimerRef.current !== null) {
        clearTimeout(copyResetTimerRef.current);
      }
    };
  }, []);

  const handleCopy = async () => {
    if (!installHint) return;
    try {
      await copyToClipboard(installHint.command);
      setCopyStatus('copied');
    } catch {
      setCopyStatus('failed');
    }

    if (copyResetTimerRef.current !== null) {
      clearTimeout(copyResetTimerRef.current);
    }
    copyResetTimerRef.current = setTimeout(() => {
      copyResetTimerRef.current = null;
      setCopyStatus('idle');
    }, 2000);
  };

  const handleOpenDocs = () => {
    if (!docsUrl) return;
    void openUrl(docsUrl);
  };

  return (
    <article
      className={`crosshook-host-tool-dashboard-tile crosshook-host-tool-dashboard-tile--${capability.state}`}
      aria-label={`${capability.label} capability status`}
    >
      <header className="crosshook-host-tool-dashboard-tile__header">
        <span className="crosshook-host-tool-dashboard-tile__dot" aria-hidden="true" />
        <h3 className="crosshook-host-tool-dashboard-tile__label">{capability.label}</h3>
      </header>

      <p
        className={`crosshook-host-tool-dashboard-tile__state crosshook-host-tool-dashboard-tile__state--${capability.state}`}
      >
        {STATE_LABEL[capability.state]}
      </p>

      <ul className="crosshook-host-tool-dashboard-tile__meta">
        <li>{formatCount(requiredMissing, requiredTotal, 'required')}</li>
        {optionalTotal > 0 ? <li>{formatCount(optionalMissing, optionalTotal, 'optional')}</li> : null}
        {capability.rationale ? (
          <li className="crosshook-host-tool-dashboard-tile__rationale">{capability.rationale}</li>
        ) : null}
      </ul>

      {installHint || docsUrl ? (
        <div className="crosshook-host-tool-dashboard-tile__actions">
          {installHint ? (
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost crosshook-button--small"
              onClick={() => void handleCopy()}
              title={`Copy install command for ${installHint.distro_family}`}
            >
              {copyStatus === 'copied' ? 'Copied!' : copyStatus === 'failed' ? 'Copy failed' : 'Copy install hint'}
            </button>
          ) : null}
          {docsUrl ? (
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost crosshook-button--small"
              onClick={handleOpenDocs}
            >
              Open docs
            </button>
          ) : null}
        </div>
      ) : null}
    </article>
  );
}

export default CapabilityTile;
