import { useMemo } from 'react';
import { useProfileContext } from '@/context/ProfileContext';
import { useLaunchSubTabsProps } from '@/hooks/launch/useLaunchSubTabsProps';
import { LaunchSubTabs } from '../../LaunchSubTabs';

export interface HeroLaunchSubTabsHostProps {
  /**
   * The name of the profile being displayed in the Hero Detail tab.
   * Passed from `HeroDetailLaunchTab` (derived from `displayProfileName` prop).
   */
  resolvedProfileName: string;
  /**
   * Steam App ID resolved from the active profile, for cover-art and ProtonDB.
   * Pass `resolveArtAppId(profile)` from the call site.
   */
  resolvedSteamAppId: string;
  /**
   * Whether the profile has been saved and is in the profile list.
   * Gates the environment autosave path.
   */
  hasSavedSelectedProfile: boolean;
  /**
   * Whether the LaunchSubTabs section should be disabled because the displayed
   * profile does not match the ProfileContext selected profile.
   *
   * When true, the sub-tabs are rendered but overlaid with a disabled hint
   * so the user understands they need to select the matching profile first.
   * LaunchStateContext (which feeds LaunchSubTabs) builds its LaunchRequest
   * from ProfileContext's *selected* profile, so writes from LaunchSubTabs
   * would target the wrong profile when the mismatch occurs.
   */
  profileMismatch: boolean;
}

/**
 * Mounts the legacy `LaunchSubTabs` component inside the Hero Detail launch
 * tab, wiring up all sub-tab panels (Environment, Gamescope, MangoHud,
 * Optimizations, Steam Options, Offline) plus ProtonDB lookup/overwrite/
 * suggestions via `useLaunchSubTabsProps`.
 *
 * NOTE(isGamescopeRunning): Task 3.1 will add `HeroLaunchGate` and wire the
 * real dep-gate here. For now we pass `false` — identical to what LaunchPage
 * passes before `useLaunchDepGate` resolves its first check. Task 3.1 owns
 * `components/library/launch/HeroLaunchGate.tsx`; this file must not create it.
 */
export function HeroLaunchSubTabsHost({
  resolvedProfileName: _resolvedProfileName,
  resolvedSteamAppId,
  hasSavedSelectedProfile,
  profileMismatch,
}: HeroLaunchSubTabsHostProps) {
  const { profileName } = useProfileContext();

  // isGamescopeRunning: Task 3.1 wires the real dep-gate (HeroLaunchGate).
  // Until then, default false — same as LaunchPage before useLaunchDepGate
  // resolves its first check. ProfileContext's selectedProfile is what matters
  // here; `resolvedProfileName` may be a fallback profile name when the
  // singleton doesn't own the game, so we use `profileName` from context for
  // the bridge hook (consistent with LaunchPage call site).
  const launchSubTabsProps = useLaunchSubTabsProps({
    isGamescopeRunning: false,
    resolvedSteamAppId,
    hasSavedSelectedProfile,
  });

  const disabledOverlay = useMemo(
    () =>
      profileMismatch ? (
        <div className="crosshook-hero-detail__subtabs-mismatch-overlay" aria-live="polite">
          <p className="crosshook-hero-detail__muted">
            Launch settings apply to the selected profile ({profileName || 'none'}). Select this game's profile to edit
            its launch settings here.
          </p>
        </div>
      ) : null,
    [profileMismatch, profileName]
  );

  return (
    <div
      className={[
        'crosshook-hero-detail__subtabs-host',
        profileMismatch ? 'crosshook-hero-detail__subtabs-host--mismatch' : '',
      ]
        .filter(Boolean)
        .join(' ')}
      aria-disabled={profileMismatch || undefined}
    >
      {disabledOverlay}
      <LaunchSubTabs {...launchSubTabsProps} />
    </div>
  );
}

export default HeroLaunchSubTabsHost;
