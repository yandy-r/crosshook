import type { LaunchMethod, LaunchRequest } from '../types';
import { LaunchPhase } from '../types';
import { useLaunchState } from '../hooks/useLaunchState';

interface LaunchPanelProps {
  profileId: string;
  method: Exclude<LaunchMethod, ''>;
  request: LaunchRequest | null;
  context?: 'default' | 'install';
}

const panelStyles = {
  card: {
    display: 'grid',
    alignContent: 'start',
    boxSizing: 'border-box',
    height: '100%',
    padding: '28px',
    borderRadius: '20px',
    background: 'rgba(14, 20, 40, 0.82)',
    border: '1px solid rgba(120, 160, 255, 0.22)',
    boxShadow: '0 24px 70px rgba(0, 0, 0, 0.38)',
    backdropFilter: 'blur(18px)',
  } as const,
};

export function LaunchPanel({ profileId, method, request, context = 'default' }: LaunchPanelProps) {
  const {
    actionLabel,
    canLaunchGame,
    canLaunchTrainer,
    errorMessage,
    helperLogPath,
    hintText,
    isBusy,
    launchGame,
    launchTrainer,
    phase,
    reset,
    statusText,
  } = useLaunchState({
    profileId,
    method,
    request,
  });

  const isWaitingForTrainer = phase === LaunchPhase.WaitingForTrainer;
  const isSessionActive = phase === LaunchPhase.SessionActive;
  const canLaunch = isWaitingForTrainer ? canLaunchTrainer : canLaunchGame;
  const primaryAction = isWaitingForTrainer ? launchTrainer : launchGame;
  const isInstallContext = context === 'install';

  if (isInstallContext) {
    return (
      <section style={panelStyles.card}>
        <div
          style={{ display: 'flex', justifyContent: 'space-between', gap: '16px', alignItems: 'start', flexWrap: 'wrap' }}
        >
          <div>
            <p
              style={{
                margin: 0,
                textTransform: 'uppercase',
                letterSpacing: '0.18em',
                color: '#7bb0ff',
                fontSize: '0.74rem',
              }}
            >
              Install Workflow
            </p>
            <h1 style={{ margin: '10px 0 6px', fontSize: '2rem', lineHeight: 1.1 }}>CrossHook Native</h1>
            <p style={{ margin: 0, color: '#9fb1d6', maxWidth: '56ch' }}>
              Install Game always targets Proton. Complete the installer flow, then review the generated profile in the modal before saving.
            </p>
          </div>

          <div
            style={{
              padding: '10px 14px',
              borderRadius: '999px',
              background: 'rgba(72, 134, 255, 0.12)',
              border: '1px solid rgba(72, 134, 255, 0.28)',
              color: '#cfe0ff',
              fontSize: '0.9rem',
              whiteSpace: 'nowrap',
            }}
          >
            Review first
          </div>
        </div>

        <div
          style={{
            marginTop: '24px',
            padding: '18px',
            borderRadius: '16px',
            background: 'rgba(7, 12, 24, 0.55)',
            border: '1px solid rgba(255, 255, 255, 0.07)',
          }}
        >
          <p style={{ margin: 0, fontWeight: 600, color: '#eef4ff' }}>
            Run the installer in the left panel, then review the generated profile in the modal before saving.
          </p>
          <p style={{ margin: '10px 0 0', color: '#9fb1d6', lineHeight: 1.6 }}>
            The install creates a reviewable draft, the final executable stays editable after candidate selection, and nothing is persisted until you save and open the Profile tab.
          </p>
        </div>

        <div style={{ marginTop: '22px', display: 'grid', gap: '10px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px', color: '#9fb1d6' }}>
            <span
              style={{
                width: '10px',
                height: '10px',
                borderRadius: '50%',
                background: '#5e77ff',
              }}
            />
            <span>Proton install flow selected</span>
          </div>
          <div style={{ color: '#7f8fb0', fontSize: '0.9rem' }}>
            Review in the modal becomes available after the installer completes and the executable is confirmed.
          </div>
        </div>
      </section>
    );
  }

  return (
    <section style={panelStyles.card}>
      <div
        style={{ display: 'flex', justifyContent: 'space-between', gap: '16px', alignItems: 'start', flexWrap: 'wrap' }}
      >
        <div>
          <p
            style={{
              margin: 0,
              textTransform: 'uppercase',
              letterSpacing: '0.18em',
              color: '#7bb0ff',
              fontSize: '0.74rem',
            }}
          >
            {method === 'steam_applaunch' ? 'Steam Launch' : method === 'proton_run' ? 'Proton Launch' : 'Native Launch'}
          </p>
          <h1 style={{ margin: '10px 0 6px', fontSize: '2rem', lineHeight: 1.1 }}>CrossHook Native</h1>
          <p style={{ margin: 0, color: '#9fb1d6', maxWidth: '56ch' }}>
            {method === 'native'
              ? 'Direct launch flow for Linux-native executables, driven by the native Tauri backend.'
              : `Two-step launch flow for ${method === 'steam_applaunch' ? 'Steam' : 'Proton'} games and trainers, driven by the native Tauri backend.`}
          </p>
        </div>

        <div
          style={{
            padding: '10px 14px',
            borderRadius: '999px',
            background: 'rgba(72, 134, 255, 0.12)',
            border: '1px solid rgba(72, 134, 255, 0.28)',
            color: '#cfe0ff',
            fontSize: '0.9rem',
            whiteSpace: 'nowrap',
          }}
        >
          {phase}
        </div>
      </div>

      <div
        style={{
          marginTop: '24px',
          padding: '18px',
          borderRadius: '16px',
          background: 'rgba(7, 12, 24, 0.55)',
          border: '1px solid rgba(255, 255, 255, 0.07)',
        }}
      >
        <p style={{ margin: 0, fontWeight: 600, color: '#eef4ff' }}>{statusText}</p>
        <p style={{ margin: '10px 0 0', color: '#9fb1d6', lineHeight: 1.6 }}>{hintText}</p>
        {helperLogPath ? (
          <p style={{ margin: '10px 0 0', color: '#7bb0ff', fontSize: '0.92rem', wordBreak: 'break-all' }}>
            Helper log: {helperLogPath}
          </p>
        ) : null}
        {errorMessage ? (
          <p style={{ margin: '10px 0 0', color: '#ff8fa3', fontSize: '0.92rem' }}>{errorMessage}</p>
        ) : null}
      </div>

      <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap', marginTop: '22px' }}>
        <button
          type="button"
          onClick={primaryAction}
          disabled={!canLaunch || isBusy}
          style={{
            minHeight: '52px',
            padding: '0 22px',
            borderRadius: '14px',
            border: '1px solid rgba(123, 176, 255, 0.45)',
            background:
              !canLaunch || isBusy ? 'rgba(123, 176, 255, 0.18)' : 'linear-gradient(135deg, #2f7cf6 0%, #5ac8fa 100%)',
            color: '#fff',
            fontSize: '1rem',
            fontWeight: 700,
            cursor: !canLaunch || isBusy ? 'not-allowed' : 'pointer',
            opacity: !canLaunch || isBusy ? 0.65 : 1,
          }}
        >
          {actionLabel}
        </button>

        <button
          type="button"
          onClick={reset}
          style={{
            minHeight: '52px',
            padding: '0 20px',
            borderRadius: '14px',
            border: '1px solid rgba(255, 255, 255, 0.12)',
            background: 'rgba(255, 255, 255, 0.04)',
            color: '#e5eefc',
            fontSize: '1rem',
            fontWeight: 600,
            cursor: 'pointer',
          }}
        >
          Reset
        </button>
      </div>

      <div style={{ marginTop: '22px', display: 'grid', gap: '10px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px', color: '#9fb1d6' }}>
          <span
            style={{
              width: '10px',
              height: '10px',
              borderRadius: '50%',
              background: isSessionActive ? '#27d17f' : isWaitingForTrainer ? '#f5c542' : '#5e77ff',
            }}
          />
          <span>
            {method === 'steam_applaunch'
              ? 'Steam runner selected'
              : method === 'proton_run'
                ? 'Proton runner selected'
                : 'Native runner selected'}
          </span>
        </div>
        <div style={{ color: '#7f8fb0', fontSize: '0.9rem' }}>
          {request ? 'Profile request is loaded.' : 'No profile request is loaded yet.'}
        </div>
      </div>
    </section>
  );
}

export default LaunchPanel;
