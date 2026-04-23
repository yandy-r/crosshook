import { HealthBadge } from '@/components/HealthBadge';
import type { InspectorBodyProps, SelectedGame } from '@/components/layout/routeMetadata';
import { useProfileContext } from '@/context/ProfileContext';
import { useProfileHealthContext } from '@/context/ProfileHealthContext';

export type GameInspectorProps = InspectorBodyProps;

function HeroSection({ selection }: { selection: SelectedGame }) {
  const title = selection.gameName || selection.name;
  return (
    <section className="crosshook-game-inspector__section" aria-labelledby="crosshook-game-inspector-hero-title">
      <h2 id="crosshook-game-inspector-hero-title" className="crosshook-game-inspector__eyebrow">
        Overview
      </h2>
      <p className="crosshook-game-inspector__title">{title}</p>
      <p className="crosshook-game-inspector__subtitle">Profile: {selection.name}</p>
    </section>
  );
}

function PillsSection({ selection }: { selection: SelectedGame }) {
  return (
    <section className="crosshook-game-inspector__section" aria-labelledby="crosshook-game-inspector-pills-title">
      <h2 id="crosshook-game-inspector-pills-title" className="crosshook-game-inspector__eyebrow">
        Status
      </h2>
      <div className="crosshook-game-inspector__pills">
        <span className="crosshook-game-inspector__pill">Steam app {selection.steamAppId || '—'}</span>
        <span className="crosshook-game-inspector__pill">
          Network: {selection.networkIsolation ? 'Isolated' : 'Default'}
        </span>
        {selection.isFavorite ? <span className="crosshook-game-inspector__pill">Favorite</span> : null}
      </div>
    </section>
  );
}

function QuickActionsSection({
  selection,
  onLaunch,
  onEditProfile,
  onToggleFavorite,
}: {
  selection: SelectedGame;
} & Pick<GameInspectorProps, 'onLaunch' | 'onEditProfile' | 'onToggleFavorite'>) {
  const displayName = selection.gameName || selection.name;
  return (
    <section className="crosshook-game-inspector__section" aria-labelledby="crosshook-game-inspector-actions-title">
      <h2 id="crosshook-game-inspector-actions-title" className="crosshook-game-inspector__eyebrow">
        Quick actions
      </h2>
      <div className="crosshook-game-inspector__actions">
        <button type="button" className="crosshook-button" onClick={() => onLaunch?.(selection.name)}>
          Launch {displayName}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--ghost"
          onClick={() => onEditProfile?.(selection.name)}
        >
          Edit profile
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--ghost"
          onClick={() => onToggleFavorite?.(selection.name, selection.isFavorite)}
        >
          {selection.isFavorite ? 'Remove favorite' : 'Add favorite'}
        </button>
      </div>
    </section>
  );
}

function ActiveProfileSection({ selection }: { selection: SelectedGame }) {
  const { profileName, profile } = useProfileContext();
  const isActive = profileName === selection.name;

  return (
    <section className="crosshook-game-inspector__section" aria-labelledby="crosshook-game-inspector-profile-title">
      <h2 id="crosshook-game-inspector-profile-title" className="crosshook-game-inspector__eyebrow">
        Active profile
      </h2>
      {!isActive ? (
        <p className="crosshook-game-inspector__muted" role="status">
          No active profile loaded for this game.
        </p>
      ) : (
        <div className="crosshook-game-inspector__profile-summary">
          <p className="crosshook-game-inspector__profile-line">
            <strong>{profileName}</strong>
          </p>
          <p className="crosshook-game-inspector__profile-line">Prefix: {profile.runtime.prefix_path || '—'}</p>
          <p className="crosshook-game-inspector__profile-line">
            Proton: {profile.runtime.proton_path || profile.steam.proton_path || '—'}
          </p>
        </div>
      )}
    </section>
  );
}

function HealthSection({ selection }: { selection: SelectedGame }) {
  const { healthByName, loading, error } = useProfileHealthContext();
  const report = healthByName[selection.name];

  if (error) {
    return (
      <section className="crosshook-game-inspector__section" aria-labelledby="crosshook-game-inspector-health-title">
        <h2 id="crosshook-game-inspector-health-title" className="crosshook-game-inspector__eyebrow">
          Health
        </h2>
        <p className="crosshook-launch-panel__feedback-help" role="status">
          {error}
        </p>
      </section>
    );
  }

  if (loading && !report) {
    return (
      <section className="crosshook-game-inspector__section" aria-labelledby="crosshook-game-inspector-health-title">
        <h2 id="crosshook-game-inspector-health-title" className="crosshook-game-inspector__eyebrow">
          Health
        </h2>
        <p className="crosshook-launch-panel__feedback-help" role="status">
          Loading health…
        </p>
      </section>
    );
  }

  if (!report) {
    return (
      <section className="crosshook-game-inspector__section" aria-labelledby="crosshook-game-inspector-health-title">
        <h2 id="crosshook-game-inspector-health-title" className="crosshook-game-inspector__eyebrow">
          Health
        </h2>
        <p className="crosshook-game-inspector__muted" role="status">
          No health data for this profile yet.
        </p>
      </section>
    );
  }

  return (
    <section className="crosshook-game-inspector__section" aria-labelledby="crosshook-game-inspector-health-title">
      <h2 id="crosshook-game-inspector-health-title" className="crosshook-game-inspector__eyebrow">
        Health
      </h2>
      <div className="crosshook-game-inspector__health-row">
        <HealthBadge report={report} metadata={report.metadata} />
        <span className="crosshook-game-inspector__health-status">{report.status}</span>
      </div>
      {report.issues.length > 0 ? (
        <ul className="crosshook-launch-panel__feedback-list" aria-label="Health issues">
          {report.issues.map((issue) => (
            <li key={`${issue.field}-${issue.path}-${issue.message}`} className="crosshook-launch-panel__feedback-item">
              <p className="crosshook-launch-panel__feedback-help">{issue.message}</p>
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}

function RecentLaunchesPlaceholder() {
  return (
    <section className="crosshook-game-inspector__section" aria-labelledby="crosshook-game-inspector-launches-title">
      <h2 id="crosshook-game-inspector-launches-title" className="crosshook-game-inspector__eyebrow">
        Recent launches
      </h2>
      {/* TODO(phase-4/follow-up): wire list_launch_history_for_profile IPC — https://github.com/yandy-r/crosshook/issues/456 */}
      <p role="status">Recent launches coming soon</p>
    </section>
  );
}

export default function GameInspector({ selection, onLaunch, onEditProfile, onToggleFavorite }: GameInspectorProps) {
  if (selection == null) {
    return (
      <p className="crosshook-game-inspector__empty" role="status">
        Select a game to see details
      </p>
    );
  }

  return (
    <div className="crosshook-game-inspector">
      <HeroSection selection={selection} />
      <PillsSection selection={selection} />
      <QuickActionsSection
        selection={selection}
        onLaunch={onLaunch}
        onEditProfile={onEditProfile}
        onToggleFavorite={onToggleFavorite}
      />
      <ActiveProfileSection selection={selection} />
      <RecentLaunchesPlaceholder />
      <HealthSection selection={selection} />
    </div>
  );
}
