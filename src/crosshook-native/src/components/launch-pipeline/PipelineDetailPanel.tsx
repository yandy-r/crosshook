import { useEffect, useId, useMemo, useState } from 'react';
import type { LaunchPreview } from '../../types/launch';
import { deriveEnvGroups, deriveWrapperChain, detailSummaryMeta } from '../../utils/derivePipelineDetail';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { EnvBreakdown } from './EnvBreakdown';
import { WrapperChain } from './WrapperChain';
import '../../styles/launch-pipeline-detail.css';

export interface PipelineDetailPanelProps {
  preview: LaunchPreview | null;
}

export function PipelineDetailPanel({ preview }: PipelineDetailPanelProps) {
  const [open, setOpen] = useState(false);
  const panelId = useId();

  useEffect(() => {
    if (preview === null) {
      setOpen(false);
    }
  }, [preview]);

  const expanded = open && preview !== null;

  const envGroups = useMemo(() => (preview ? deriveEnvGroups(preview) : []), [preview]);
  const chain = useMemo(() => (preview ? deriveWrapperChain(preview) : null), [preview]);
  const summaryMeta = useMemo(
    () => (preview ? detailSummaryMeta(envGroups, chain, preview.umu_decision) : ''),
    [preview, envGroups, chain]
  );

  return (
    <div className="crosshook-pipeline-detail">
      <button
        type="button"
        className="crosshook-pipeline-detail__toggle"
        aria-expanded={expanded}
        aria-controls={expanded ? panelId : undefined}
        disabled={preview === null}
        data-open={expanded}
        onClick={() => setOpen((value) => !value)}
      >
        <span className="crosshook-pipeline-detail__toggle-chevron" aria-hidden="true" />
        <span className="crosshook-pipeline-detail__toggle-label">Pipeline details</span>
        {summaryMeta ? <span className="crosshook-pipeline-detail__toggle-meta">{summaryMeta}</span> : null}
      </button>
      {preview === null ? <p className="crosshook-pipeline-detail__hint">Run Dry-run to inspect the pipeline</p> : null}
      {expanded && preview ? (
        <section id={panelId} aria-label="Launch pipeline details" className="crosshook-pipeline-detail__panel">
          {preview.directives_error && preview.environment === null ? (
            <p className="crosshook-pipeline-detail__error" role="alert">
              {preview.directives_error}
            </p>
          ) : null}
          <CollapsibleSection title="Wrapper chain" defaultOpen className="crosshook-pipeline-detail__section">
            {chain === null ? (
              <p className="crosshook-wrapper-chain__empty">Wrapper details unavailable for this preview.</p>
            ) : (
              <WrapperChain items={chain} emptyMessage="No wrappers for native launches" />
            )}
          </CollapsibleSection>
          <CollapsibleSection title="Environment" defaultOpen className="crosshook-pipeline-detail__section">
            <EnvBreakdown groups={envGroups} />
          </CollapsibleSection>
        </section>
      ) : null}
    </div>
  );
}

export default PipelineDetailPanel;
