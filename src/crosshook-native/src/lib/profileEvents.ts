import type { ProfileChangeReason, ProfilesChangedPayload } from '../types/profile-events';

type UnitReason = Exclude<ProfileChangeReason, { kind: 'renamed' }>;

/** Reasons that carry no fields besides `kind`; `null` for anything else. */
function unitReason(kind: string): UnitReason | null {
  switch (kind) {
    case 'saved':
    case 'deleted':
    case 'duplicated':
    case 'favorite_toggled':
    case 'rolled_back':
    case 'preset_applied':
    case 'imported':
    case 'migrated':
      return { kind };
    default:
      return null;
  }
}

function parseReason(value: unknown): ProfileChangeReason | null {
  if (typeof value !== 'object' || value === null || !('kind' in value) || typeof value.kind !== 'string') {
    return null;
  }
  if (value.kind === 'renamed') {
    return 'from' in value && typeof value.from === 'string' ? { kind: 'renamed', from: value.from } : null;
  }
  return unitReason(value.kind);
}

/**
 * Validates a `profiles-changed` payload at the event boundary. Returns `null`
 * for anything malformed; callers treat `null` as a conservative resync
 * (refresh everything, including health) rather than skipping work.
 */
export function parseProfilesChanged(payload: unknown): ProfilesChangedPayload | null {
  if (typeof payload !== 'object' || payload === null || !('reason' in payload) || !('names' in payload)) {
    return null;
  }
  const reason = parseReason(payload.reason);
  const { names } = payload;
  if (reason === null || !Array.isArray(names) || !names.every((name) => typeof name === 'string')) {
    return null;
  }
  return { reason, names: [...names] };
}

/** Whether a change can affect profile health. Only favorite toggles cannot. */
export function profileChangeAffectsHealth(reason: ProfileChangeReason): boolean {
  return reason.kind !== 'favorite_toggled';
}
