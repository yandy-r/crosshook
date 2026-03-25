import type { InstallGameExecutableCandidate, ProfileReviewSource } from './install';
import type { GameProfile } from './profile';

export interface ProfileReviewSession {
  isOpen: boolean;
  source: ProfileReviewSource;
  profileName: string;
  originalProfile: GameProfile;
  draftProfile: GameProfile;
  candidateOptions: InstallGameExecutableCandidate[];
  helperLogPath: string;
  installMessage: string;
  dirty: boolean;
  saveError: string | null;
}
