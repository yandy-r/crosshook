import type { InstallGameExecutableCandidate, ProfileReviewSource } from './install';
import type { GameProfile } from './profile';

export interface ProfileReviewSession {
  isOpen: boolean;
  source: ProfileReviewSource;
  profileName: string;
  /** Snapshot name when the session was created (for derived dirty state). */
  originalProfileName: string;
  originalProfile: GameProfile;
  draftProfile: GameProfile;
  candidateOptions: InstallGameExecutableCandidate[];
  helperLogPath: string;
  installMessage: string;
  saveError: string | null;
}
