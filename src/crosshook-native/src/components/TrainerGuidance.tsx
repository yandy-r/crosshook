import { CollapsibleSection } from './ui/CollapsibleSection';
import type { TrainerGuidanceContent } from '../types/onboarding';

type TrainerLoadingMode = 'source_directory' | 'copy_to_prefix';

interface TrainerGuidanceProps {
  selectedMode: TrainerLoadingMode;
  onModeChange: (mode: TrainerLoadingMode) => void;
  guidanceContent: TrainerGuidanceContent;
}

export function TrainerGuidance({ selectedMode, onModeChange, guidanceContent }: TrainerGuidanceProps) {
  const sourceDirectoryEntry = guidanceContent.loading_modes.find((m) => m.id === 'source_directory');
  const copyToPrefixEntry = guidanceContent.loading_modes.find((m) => m.id === 'copy_to_prefix');
  const flingEntry = guidanceContent.trainer_sources.find((s) => s.id === 'fling');
  const wemodEntry = guidanceContent.trainer_sources.find((s) => s.id === 'wemod');

  return (
    <div className="crosshook-trainer-guidance">
      <h3 className="crosshook-trainer-guidance__heading">Trainer Loading Mode</h3>
      <p className="crosshook-trainer-guidance__subheading">
        Choose how CrossHook stages the trainer when launching your game.
      </p>

      <div className="crosshook-trainer-guidance__mode-cards">
        <LoadingModeCard
          id="source_directory"
          title={sourceDirectoryEntry?.title ?? 'Source Directory'}
          summary={sourceDirectoryEntry?.description ?? 'Proton reads the trainer directly from its downloaded location.'}
          whenToUse={sourceDirectoryEntry?.when_to_use}
          examples={sourceDirectoryEntry?.examples}
          isSelected={selectedMode === 'source_directory'}
          onSelect={() => onModeChange('source_directory')}
          badge="Recommended"
        />

        <LoadingModeCard
          id="copy_to_prefix"
          title={copyToPrefixEntry?.title ?? 'Copy to Prefix'}
          summary={copyToPrefixEntry?.description ?? "CrossHook copies the trainer into the WINE prefix's C:\\ drive before launch."}
          whenToUse={copyToPrefixEntry?.when_to_use}
          examples={copyToPrefixEntry?.examples}
          isSelected={selectedMode === 'copy_to_prefix'}
          onSelect={() => onModeChange('copy_to_prefix')}
          hint="FLiNG trainers that bundle DLLs work best with Copy to Prefix"
        />
      </div>

      {guidanceContent.trainer_sources.length > 0 ? (
        <div className="crosshook-trainer-guidance__sources">
          <h4 className="crosshook-trainer-guidance__sources-heading">Trainer Sources</h4>
          <div className="crosshook-trainer-guidance__source-list">
            {flingEntry ? (
              <TrainerSourceCard
                title={flingEntry.title}
                description={flingEntry.description}
                whenToUse={flingEntry.when_to_use}
              />
            ) : null}
            {wemodEntry ? (
              <TrainerSourceCard
                title={wemodEntry.title}
                description={wemodEntry.description}
                whenToUse={wemodEntry.when_to_use}
                disclaimer="WeMod requires its own desktop app installed under WINE — see wemod-launcher"
              />
            ) : null}
          </div>
        </div>
      ) : null}

      <div className="crosshook-panel crosshook-trainer-guidance__av-warning" role="note">
        <span className="crosshook-trainer-guidance__av-warning-icon" aria-hidden="true">⚠</span>
        <p className="crosshook-trainer-guidance__av-warning-text">
          Some antivirus tools may flag trainer executables — this is a known false positive with game trainers.
        </p>
      </div>
    </div>
  );
}

interface LoadingModeCardProps {
  id: TrainerLoadingMode;
  title: string;
  summary: string;
  whenToUse?: string;
  examples?: string[];
  isSelected: boolean;
  onSelect: () => void;
  badge?: string;
  hint?: string;
}

function LoadingModeCard({
  id,
  title,
  summary,
  whenToUse,
  examples,
  isSelected,
  onSelect,
  badge,
  hint,
}: LoadingModeCardProps) {
  const hasLearnMore = Boolean(whenToUse || (examples && examples.length > 0));

  return (
    <button
      type="button"
      className={[
        'crosshook-panel',
        'crosshook-trainer-guidance__mode-card',
        isSelected ? 'crosshook-trainer-guidance__mode-card--selected' : '',
      ]
        .filter(Boolean)
        .join(' ')}
      aria-pressed={isSelected}
      onClick={onSelect}
      style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
    >
      <div className="crosshook-trainer-guidance__mode-card-header">
        <div className="crosshook-trainer-guidance__mode-card-indicator" aria-hidden="true">
          {isSelected ? '●' : '○'}
        </div>
        <div className="crosshook-trainer-guidance__mode-card-title-row">
          <span className="crosshook-trainer-guidance__mode-card-title">{title}</span>
          {badge ? (
            <span className="crosshook-trainer-guidance__mode-card-badge">{badge}</span>
          ) : null}
        </div>
      </div>

      <p className="crosshook-trainer-guidance__mode-card-summary">{summary}</p>

      {hint ? (
        <p className="crosshook-trainer-guidance__mode-card-hint" aria-live="polite">
          {hint}
        </p>
      ) : null}

      {hasLearnMore ? (
        <CollapsibleSection
          title="Learn more"
          defaultOpen={false}
          className="crosshook-trainer-guidance__mode-card-details"
        >
          {whenToUse ? (
            <p className="crosshook-trainer-guidance__details-when">
              <strong>When to use:</strong> {whenToUse}
            </p>
          ) : null}
          {examples && examples.length > 0 ? (
            <ul className="crosshook-trainer-guidance__details-examples">
              {examples.map((example) => (
                <li key={example}>{example}</li>
              ))}
            </ul>
          ) : null}
        </CollapsibleSection>
      ) : null}
    </button>
  );
}

interface TrainerSourceCardProps {
  title: string;
  description: string;
  whenToUse: string;
  disclaimer?: string;
}

function TrainerSourceCard({ title, description, whenToUse, disclaimer }: TrainerSourceCardProps) {
  return (
    <div className="crosshook-panel crosshook-trainer-guidance__source-card">
      <div className="crosshook-trainer-guidance__source-card-title">{title}</div>
      <p className="crosshook-trainer-guidance__source-card-description">{description}</p>
      {disclaimer ? (
        <p className="crosshook-trainer-guidance__source-card-disclaimer">{disclaimer}</p>
      ) : null}
      <CollapsibleSection
        title="When to use"
        defaultOpen={false}
        className="crosshook-trainer-guidance__source-card-details"
      >
        <p>{whenToUse}</p>
      </CollapsibleSection>
    </div>
  );
}

export default TrainerGuidance;
