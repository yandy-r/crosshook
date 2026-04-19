import type { ReactNode } from 'react';
import type { LaunchMethod, LaunchRequest } from '../../types';
import type { GameProfile } from '../../types/profile';

export interface LaunchPanelProps {
  profileId: string;
  method: Exclude<LaunchMethod, ''>;
  request: LaunchRequest | null;
  profile: GameProfile;
  /** Profile dropdown (placed in the top row next to Launch / Preview / Reset). */
  profileSelectSlot?: ReactNode;
  /** @deprecated Use `profileSelectSlot` — kept for call sites not yet migrated. */
  beforeActions?: ReactNode;
  /** Slot rendered where the info/status area is (e.g. pinned profiles). */
  infoSlot?: ReactNode;
  /** Slot rendered between the controls card and the actions card (e.g. tabbed config panels). */
  tabsSlot?: ReactNode;
  /**
   * Optional pre-launch gate. Called before launchGame/launchTrainer.
   * Return true to proceed, false to abort (e.g. show a modal first).
   */
  onBeforeLaunch?: (action: 'game' | 'trainer') => Promise<boolean>;
}
