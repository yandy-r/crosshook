import { InstallField } from '../ui/InstallField';
import { WizardReviewSummary } from '../wizard/WizardReviewSummary';
import type { WizardValidationResult } from '../wizard/wizardValidation';
import type {
  InstallGameExecutableCandidate,
  InstallGamePrefixPathState,
  InstallGameStage,
} from '../../types/install';

export interface InstallationStatus {
  stage: InstallGameStage;
  statusText: string;
  hintText: string;
  error: string | null;
  generalError: string | null;
  candidateOptions: readonly InstallGameExecutableCandidate[];
  currentExecutablePath: string;
  onSelectCandidate: (path: string) => void;
  onFinalExecutableChange: (path: string) => void;
  finalExecutableError: string | null | undefined;
  helperLogPath: string;
  isRunningInstaller: boolean;
  defaultPrefixPathState: InstallGamePrefixPathState;
  candidateCount: number;
}

export interface InstallReviewSummaryProps {
  installation: InstallationStatus;
  validation: WizardValidationResult;
}

function stageLabel(stage: InstallGameStage): string {
  switch (stage) {
    case 'preparing':
      return 'Preparing';
    case 'running_installer':
      return 'Running installer';
    case 'review_required':
      return 'Review required';
    case 'ready_to_save':
      return 'Ready to save';
    case 'failed':
      return 'Failed';
    case 'idle':
    default:
      return 'Idle';
  }
}

function prefixStateLabel(state: InstallGamePrefixPathState): string {
  switch (state) {
    case 'loading':
      return 'Resolving default prefix...';
    case 'ready':
      return 'Default prefix resolved';
    case 'failed':
      return 'Default prefix unavailable';
    case 'idle':
    default:
      return 'Awaiting profile name';
  }
}

function fileNameFromPath(path: string): string {
  const normalized = path.trim().replace(/\\/g, '/');
  const lastSegment = normalized.split('/').pop() ?? '';
  return lastSegment || normalized || 'Unnamed executable';
}

function candidateLabel(candidate: InstallGameExecutableCandidate): string {
  const baseName = fileNameFromPath(candidate.path);
  return candidate.is_recommended ? `${baseName} (recommended)` : baseName;
}

export function InstallReviewSummary({ installation, validation }: InstallReviewSummaryProps) {
  const {
    stage,
    statusText,
    hintText,
    error,
    generalError,
    candidateOptions,
    currentExecutablePath,
    onSelectCandidate,
    onFinalExecutableChange,
    finalExecutableError,
    helperLogPath,
    isRunningInstaller,
    defaultPrefixPathState,
    candidateCount,
  } = installation;

  return (
    <div className="crosshook-install-card">
      <div className="crosshook-install-status">
        <div>
          <div className="crosshook-install-stage">{stageLabel(stage)}</div>
          <p className="crosshook-heading-copy" style={{ marginTop: 8 }}>
            {statusText}
          </p>
        </div>
        <div style={{ display: 'grid', gap: 10, justifyItems: 'end' }}>
          <div className="crosshook-install-pill">{prefixStateLabel(defaultPrefixPathState)}</div>
          <div className="crosshook-install-pill">Candidates: {candidateCount}</div>
        </div>
      </div>

      <div className="crosshook-install-review">
        {error ? <p className="crosshook-danger">{error}</p> : null}
        {generalError ? <p className="crosshook-danger">{generalError}</p> : null}
        <p className="crosshook-help-text">{hintText}</p>

        <InstallField
          label="Final Executable"
          value={currentExecutablePath}
          onChange={onFinalExecutableChange}
          placeholder="/home/user/.local/share/crosshook/prefixes/example/drive_c/Game/Game.exe"
          browseLabel="Browse"
          browseTitle="Select Installed Game Executable"
          browseFilters={[{ name: 'Windows Executable', extensions: ['exe'] }]}
          helpText="Selecting a candidate fills this field; it stays editable for the final review step."
          error={finalExecutableError}
        />

        {candidateOptions.length > 0 ? (
          <div className="crosshook-install-candidate-list">
            {candidateOptions.map((candidate) => {
              const isSelected = candidate.path.trim() === currentExecutablePath.trim();
              return (
                <button
                  key={`${candidate.index}:${candidate.path}`}
                  type="button"
                  className="crosshook-install-candidate"
                  onClick={() => onSelectCandidate(candidate.path)}
                  style={{
                    width: '100%',
                    appearance: 'none',
                    textAlign: 'left',
                    cursor: 'pointer',
                    color: isSelected ? 'var(--crosshook-color-text)' : 'var(--crosshook-color-text-muted)',
                    borderColor: isSelected ? 'rgba(0, 120, 212, 0.45)' : 'rgba(255, 255, 255, 0.06)',
                  }}
                >
                  <span>
                    <strong style={{ color: 'var(--crosshook-color-text)' }}>{candidateLabel(candidate)}</strong>
                    {candidate.is_recommended ? <span className="crosshook-muted"> - suggested</span> : null}
                  </span>
                  <span style={{ wordBreak: 'break-all' }}>{candidate.path}</span>
                </button>
              );
            })}
          </div>
        ) : (
          <p className="crosshook-help-text">
            {isRunningInstaller
              ? 'Candidate discovery will appear after the installer exits.'
              : 'Run the installer to discover candidate executables.'}
          </p>
        )}

        {helperLogPath ? (
          <div className="crosshook-install-candidate" style={{ cursor: 'default', flexDirection: 'column' }}>
            <span>Installer log path</span>
            <span style={{ wordBreak: 'break-all', color: 'var(--crosshook-color-text)' }}>{helperLogPath}</span>
          </div>
        ) : null}
      </div>

      <WizardReviewSummary validation={validation} readinessResult={null} checkError={null} />
    </div>
  );
}

export default InstallReviewSummary;
