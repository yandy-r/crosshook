import { useEffect, useRef, useState } from 'react';
import { copyToClipboard } from '../../utils/clipboard';
import { type EnvGroup, formatEnvGroupClipboard } from '../../utils/derivePipelineDetail';
import { CollapsibleSection } from '../ui/CollapsibleSection';

export interface EnvBreakdownProps {
  groups: EnvGroup[];
}

const COPY_STATUS_RESET_MS = 2500;

export function EnvBreakdown({ groups }: EnvBreakdownProps) {
  const [copyStatus, setCopyStatus] = useState<string | null>(null);
  const copyResetRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (copyResetRef.current) {
        clearTimeout(copyResetRef.current);
      }
    };
  }, []);

  if (groups.length === 0) {
    return <p className="crosshook-env-group__empty">No environment variables.</p>;
  }

  const handleCopy = async (group: EnvGroup): Promise<void> => {
    try {
      await copyToClipboard(formatEnvGroupClipboard(group));
      setCopyStatus(`Copied ${group.vars.length} ${group.label} variable${group.vars.length === 1 ? '' : 's'}.`);
    } catch {
      setCopyStatus('Failed to copy.');
    }
    if (copyResetRef.current) {
      clearTimeout(copyResetRef.current);
    }
    copyResetRef.current = setTimeout(() => {
      setCopyStatus(null);
      copyResetRef.current = null;
    }, COPY_STATUS_RESET_MS);
  };

  return (
    <div className="crosshook-env-breakdown">
      {groups.map((group) => (
        <CollapsibleSection
          key={group.id}
          title={group.label}
          defaultOpen={true}
          meta={`${group.vars.length} var${group.vars.length === 1 ? '' : 's'}`}
          className={group.cleared ? 'crosshook-env-group crosshook-env-group--cleared' : 'crosshook-env-group'}
        >
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary crosshook-env-group__copy"
            onClick={() => void handleCopy(group)}
          >
            Copy
          </button>
          <dl className="crosshook-env-group__list">
            {group.vars.map((envVar) => (
              <div className="crosshook-env-group__row" key={envVar.key}>
                <dt className="crosshook-env-group__key">{envVar.key}</dt>
                <dd className="crosshook-env-group__value" title={envVar.value ?? undefined}>
                  {group.cleared ? <span className="crosshook-env-group__unset-badge">unset</span> : envVar.value}
                </dd>
              </div>
            ))}
          </dl>
        </CollapsibleSection>
      ))}
      <p className="crosshook-env-group__copy-status" role="status" aria-atomic="true">
        {copyStatus}
      </p>
    </div>
  );
}

export default EnvBreakdown;
