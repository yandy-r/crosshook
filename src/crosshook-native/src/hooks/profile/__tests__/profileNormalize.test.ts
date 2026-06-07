import { describe, expect, it } from 'vitest';
import { createDefaultProfile } from '@/types/profile';
import { createEmptyProfile } from '../createEmptyProfile';
import { normalizeProfileForEdit } from '../profileNormalize';

describe('profile runtime umu hints', () => {
  it('initializes empty profile helpers with umu hint fields', () => {
    expect(createDefaultProfile().runtime).toMatchObject({
      umu_store: '',
      umu_codename: '',
    });
    expect(createEmptyProfile().runtime).toMatchObject({
      umu_store: '',
      umu_codename: '',
    });
  });

  it('normalizes umu store and codename hints for edit/save', () => {
    const profile = createDefaultProfile();
    profile.runtime.umu_store = '  GOG  ';
    profile.runtime.umu_codename = '  Cyberpunk_2077  ';

    const normalized = normalizeProfileForEdit(profile, {}, false);

    expect(normalized.runtime.umu_store).toBe('gog');
    expect(normalized.runtime.umu_codename).toBe('Cyberpunk_2077');
  });

  it('drops umu hints with control characters and caps long values', () => {
    const profile = createDefaultProfile();
    profile.runtime.umu_store = 'go\ng';
    profile.runtime.umu_codename = 'x'.repeat(140);

    const normalized = normalizeProfileForEdit(profile, {}, false);

    expect(normalized.runtime.umu_store).toBe('');
    expect(normalized.runtime.umu_codename).toHaveLength(128);
  });
});
