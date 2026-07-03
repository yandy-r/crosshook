import { useEffect, useId, useRef, useState } from 'react';
import { useProfileMods } from '@/hooks/useProfileMods';
import type { ProfileModInput, ProfileModRecord } from '@/types/mods';
import { ModAdvisoryList } from './ModAdvisoryList';
import { ModDetectionReviewModal } from './ModDetectionReviewModal';
import { ModForm } from './ModForm';
import { ModList } from './ModList';

export interface HeroModsSectionProps {
  profileName: string | undefined;
  hasTrainerConfigured: boolean;
  gameExecutablePath: string;
  /** Changes when any advisory input changes (trainer, launch method, overlays, injection) — retriggers advisories. */
  trainerSignature?: string;
}

export function HeroModsSection({
  profileName,
  hasTrainerConfigured,
  gameExecutablePath,
  trainerSignature,
}: HeroModsSectionProps) {
  const scanHintId = useId();
  const scanButtonRef = useRef<HTMLButtonElement>(null);
  const {
    mods,
    error,
    unavailable,
    addMod,
    updateMod,
    removeMod,
    toggleMod,
    detect,
    advisories,
    advisoriesError,
    refreshAdvisories,
  } = useProfileMods(profileName);

  const [adding, setAdding] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [announcement, setAnnouncement] = useState('');
  const [reviewOpen, setReviewOpen] = useState(false);

  const hasGamePath = gameExecutablePath.trim().length > 0;
  const hasMods = (mods?.length ?? 0) > 0;
  const hasEnabledMods = (mods ?? []).some((mod) => mod.enabled);

  useEffect(() => {
    if (trainerSignature !== undefined && hasMods) {
      refreshAdvisories();
    }
  }, [trainerSignature, hasMods, refreshAdvisories]);

  useEffect(() => {
    if (detect.report) {
      setReviewOpen(true);
      const count = detect.report.candidates.length;
      setAnnouncement(
        count > 0
          ? `Scan complete: ${count} candidate${count === 1 ? '' : 's'} found.`
          : 'Scan complete: no candidates found.'
      );
    }
  }, [detect.report]);

  async function handleAdd(input: ProfileModInput) {
    await addMod(input);
    setAdding(false);
    setAnnouncement(`${input.name} registered.`);
  }

  async function handleEdit(mod: ProfileModRecord, input: ProfileModInput) {
    await updateMod(mod.mod_id, input);
    setEditingId(null);
    setAnnouncement('Mod updated.');
  }

  async function handleRemove(modId: string) {
    try {
      await removeMod(modId);
      setAnnouncement('Mod removed.');
    } catch (e) {
      setAnnouncement(e instanceof Error ? e.message : String(e));
    }
  }

  async function handleRegisterCandidates(inputs: ProfileModInput[]): Promise<{ name: string; error: string }[]> {
    const failures: { name: string; error: string }[] = [];
    for (const input of inputs) {
      try {
        await addMod(input);
        setAnnouncement(`${input.name} registered.`);
      } catch (e) {
        failures.push({ name: input.name, error: e instanceof Error ? e.message : String(e) });
      }
    }
    if (inputs.length > failures.length) {
      refreshAdvisories();
    }
    return failures;
  }

  function closeReview() {
    setReviewOpen(false);
    detect.clear();
    scanButtonRef.current?.focus();
  }

  if (unavailable) {
    return (
      <div className="crosshook-mods">
        <p className="crosshook-mods__notice" role="status">
          Mod registry is unavailable — the metadata database could not be opened. Launches will proceed without
          coexistence advisories.
        </p>
      </div>
    );
  }

  const scanButton = (
    <>
      <button
        ref={scanButtonRef}
        type="button"
        className="crosshook-button crosshook-button--ghost"
        disabled={!hasGamePath || detect.scanning}
        aria-describedby={hasGamePath ? undefined : scanHintId}
        aria-busy={detect.scanning}
        onClick={() => void detect.run()}
      >
        {detect.scanning ? 'Scanning…' : 'Scan game directory'}
      </button>
      {!hasGamePath ? (
        <span id={scanHintId} className="crosshook-mods__hint">
          Set the game executable path to enable detection.
        </span>
      ) : null}
    </>
  );

  return (
    <div className="crosshook-mods">
      <span className="crosshook-visually-hidden" aria-live="polite" aria-atomic="true">
        {announcement}
      </span>

      {mods === null ? (
        <p className="crosshook-mods__notice">Loading mod registry…</p>
      ) : mods.length === 0 && !adding ? (
        <div className="crosshook-mods__empty">
          <p>No mods registered for this game yet.</p>
          <div className="crosshook-mods__toolbar">
            <button type="button" className="crosshook-button" onClick={() => setAdding(true)}>
              Add mod
            </button>
            {scanButton}
          </div>
        </div>
      ) : (
        <>
          <div className="crosshook-mods__toolbar">
            <button
              type="button"
              className="crosshook-button"
              aria-expanded={adding}
              onClick={() => {
                setAdding((current) => !current);
                setEditingId(null);
              }}
            >
              {adding ? 'Close form' : 'Add mod'}
            </button>
            {scanButton}
          </div>

          {adding ? (
            <ModForm gameExecutablePath={gameExecutablePath} onSubmit={handleAdd} onCancel={() => setAdding(false)} />
          ) : null}

          {mods.length > 0 ? (
            <ModList
              mods={mods}
              editingId={editingId}
              onToggle={(mod) => void toggleMod(mod)}
              onEdit={(modId) => {
                setAdding(false);
                setEditingId((current) => (current === modId ? null : modId));
              }}
              onRemove={(modId) => void handleRemove(modId)}
              renderEditor={(mod) => (
                <ModForm
                  initial={mod}
                  gameExecutablePath={gameExecutablePath}
                  onSubmit={(input) => handleEdit(mod, input)}
                  onCancel={() => setEditingId(null)}
                />
              )}
            />
          ) : null}

          {hasMods && !hasTrainerConfigured ? (
            <p className="crosshook-mods__hint" role="status">
              No trainer loading mode configured — advisories apply once a trainer is set.
            </p>
          ) : null}

          <ModAdvisoryList advisories={advisories} hasEnabledMods={hasEnabledMods} />
          {advisoriesError ? (
            <p className="crosshook-mods__hint" role="status">
              Advisories could not be computed: {advisoriesError}
            </p>
          ) : null}
        </>
      )}

      {error ? (
        <p className="crosshook-mods-form__error" role="alert">
          {error}
        </p>
      ) : null}
      {detect.error ? (
        <p className="crosshook-mods__hint" role="status">
          {detect.error}
        </p>
      ) : null}

      {reviewOpen && detect.report ? (
        <ModDetectionReviewModal report={detect.report} onRegister={handleRegisterCandidates} onClose={closeReview} />
      ) : null}
    </div>
  );
}
