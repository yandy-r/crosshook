import { type MouseEvent, useId, useRef, useState } from 'react';
import { useFocusTrap } from '@/hooks/useFocusTrap';
import type { DetectedModCandidate, DetectionScanReport, ProfileModInput } from '@/types/mods';
import { candidateToInput, MOD_CATEGORY_LABELS } from './mods-model';

export interface ModDetectionReviewModalProps {
  report: DetectionScanReport;
  /** Registers the selected candidates; returns per-candidate failures. */
  onRegister: (inputs: ProfileModInput[]) => Promise<{ name: string; error: string }[]>;
  onClose: () => void;
}

export function ModDetectionReviewModal({ report, onRegister, onClose }: ModDetectionReviewModalProps) {
  const titleId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const { handleKeyDown } = useFocusTrap({ open: true, panelRef, onClose });

  const [checked, setChecked] = useState<Set<string>>(
    () => new Set(report.candidates.filter((c) => !c.already_registered).map((c) => c.detector_id))
  );
  const [failures, setFailures] = useState<{ name: string; error: string }[]>([]);
  const [busy, setBusy] = useState(false);

  const selectable = report.candidates.filter((c) => !c.already_registered);
  const selected = selectable.filter((c) => checked.has(c.detector_id));

  function toggleCandidate(candidate: DetectedModCandidate) {
    if (candidate.already_registered) {
      return;
    }
    setChecked((current) => {
      const next = new Set(current);
      if (next.has(candidate.detector_id)) {
        next.delete(candidate.detector_id);
      } else {
        next.add(candidate.detector_id);
      }
      return next;
    });
  }

  async function handleRegister() {
    if (selected.length === 0) {
      return;
    }
    setBusy(true);
    try {
      const results = await onRegister(selected.map(candidateToInput));
      setFailures(results);
      const failedNames = new Set(results.map((r) => r.name));
      setChecked((current) => {
        const next = new Set<string>();
        for (const candidate of selected) {
          if (failedNames.has(candidate.suggested_name) && current.has(candidate.detector_id)) {
            next.add(candidate.detector_id);
          }
        }
        return next;
      });
      if (results.length === 0) {
        onClose();
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="crosshook-modal crosshook-mods-review" role="presentation">
      <div
        className="crosshook-modal__backdrop"
        aria-hidden="true"
        onMouseDown={(e: MouseEvent<HTMLDivElement>) => {
          if (e.target === e.currentTarget && !busy) {
            onClose();
          }
        }}
      />
      <div
        ref={panelRef}
        className="crosshook-modal__surface crosshook-panel crosshook-focus-scope crosshook-mods-review__surface"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onKeyDown={handleKeyDown}
      >
        <header className="crosshook-modal__header">
          <div className="crosshook-modal__heading-block">
            <h2 id={titleId} className="crosshook-modal__title">
              Review detected mods
            </h2>
          </div>
          <div className="crosshook-modal__header-actions">
            <button type="button" className="crosshook-button crosshook-button--ghost" onClick={onClose}>
              Cancel
            </button>
          </div>
        </header>
        <div className="crosshook-modal__body crosshook-mods-review__body">
          {report.truncated ? (
            <p className="crosshook-mods-review__notice" role="status">
              Scan stopped at the entry budget — deep mod folders may be missing. Register anything missing manually.
            </p>
          ) : null}
          {report.candidates.length === 0 ? (
            <p className="crosshook-mods-review__empty">No known mod artifacts were found in the game directory.</p>
          ) : (
            <ul className="crosshook-mods-review__list">
              {report.candidates.map((candidate) => (
                <li key={candidate.detector_id} className="crosshook-mods-review__item">
                  <label className="crosshook-mods-review__row">
                    <input
                      type="checkbox"
                      checked={checked.has(candidate.detector_id)}
                      disabled={candidate.already_registered || busy}
                      onChange={() => toggleCandidate(candidate)}
                    />
                    <span className="crosshook-mods-review__name">{candidate.suggested_name}</span>
                    <span className="crosshook-mods-list__chip">{MOD_CATEGORY_LABELS[candidate.category]}</span>
                    {candidate.already_registered ? (
                      <span className="crosshook-mods-review__registered">Already registered</span>
                    ) : null}
                  </label>
                  <p className="crosshook-mods-review__paths" title={candidate.matched_paths.join('\n')}>
                    {candidate.matched_paths.join(', ')}
                  </p>
                </li>
              ))}
            </ul>
          )}
          {failures.length > 0 ? (
            <div className="crosshook-mods-review__failures" role="status">
              {failures.map((failure) => (
                <p key={failure.name}>
                  {failure.name}: {failure.error}
                </p>
              ))}
            </div>
          ) : null}
        </div>
        <footer className="crosshook-mods-review__footer">
          <button
            type="button"
            className="crosshook-button"
            disabled={selected.length === 0 || busy}
            aria-busy={busy}
            onClick={handleRegister}
          >
            Register {selected.length} selected
          </button>
        </footer>
      </div>
    </div>
  );
}
