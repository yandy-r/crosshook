import { describe, expect, it } from 'vitest';
import type { DetectedModCandidate, ModCategory, ProfileModRecord } from '@/types/mods';
import {
  candidateToInput,
  isHttpUrl,
  MOD_CATEGORY_LABELS,
  MOD_CATEGORY_OPTIONS,
  sortMods,
  summarizeModPaths,
} from '../mods/mods-model';

function buildMod(overrides: Partial<ProfileModRecord> = {}): ProfileModRecord {
  return {
    mod_id: 'mod-1',
    profile_id: 'profile-1',
    name: 'ReShade',
    category: 'overlay_injection',
    paths: [],
    enabled: true,
    provenance: 'manual',
    created_at: '2026-01-01T00:00:00+00:00',
    updated_at: '2026-01-01T00:00:00+00:00',
    ...overrides,
  };
}

describe('mods-model', () => {
  it('category labels exhaustive', () => {
    const categories: ModCategory[] = ['overlay_injection', 'script_extender', 'file_replacement', 'other'];
    expect(Object.keys(MOD_CATEGORY_LABELS).sort()).toEqual([...categories].sort());
    expect(MOD_CATEGORY_OPTIONS).toHaveLength(categories.length);
    for (const option of MOD_CATEGORY_OPTIONS) {
      expect(option.label).toBe(MOD_CATEGORY_LABELS[option.value]);
      expect(option.label.length).toBeGreaterThan(0);
    }
  });

  it('summarizeModPaths basename and +N more', () => {
    expect(summarizeModPaths([])).toEqual({ summary: '', full: '' });
    expect(summarizeModPaths(['dir/sub/a.dll'])).toEqual({ summary: 'a.dll', full: 'dir/sub/a.dll' });
    expect(summarizeModPaths(['C:\\Games\\enb.ini'])).toEqual({ summary: 'enb.ini', full: 'C:\\Games\\enb.ini' });
    expect(summarizeModPaths(['a.dll', 'b.ini', 'c.dll'])).toEqual({
      summary: 'a.dll +2 more',
      full: 'a.dll\nb.ini\nc.dll',
    });
  });

  it('sortMods enabled category name', () => {
    const mods = [
      buildMod({ mod_id: 'd', name: 'zeta tool', category: 'other', enabled: false }),
      buildMod({ mod_id: 'c', name: 'Beta Extender', category: 'script_extender' }),
      buildMod({ mod_id: 'b', name: 'alpha extender', category: 'script_extender' }),
      buildMod({ mod_id: 'a', name: 'Overlay', category: 'overlay_injection' }),
    ];

    expect(sortMods(mods).map((mod) => mod.mod_id)).toEqual(['a', 'b', 'c', 'd']);
    // Input order untouched (pure).
    expect(mods[0].mod_id).toBe('d');
  });

  it('candidateToInput shape', () => {
    const candidate: DetectedModCandidate = {
      detector_id: 'script_extender:skse64',
      suggested_name: 'SKSE64',
      category: 'script_extender',
      matched_paths: ['skse64_loader.exe', 'Data/SKSE'],
      already_registered: false,
    };

    expect(candidateToInput(candidate)).toEqual({
      name: 'SKSE64',
      category: 'script_extender',
      paths: ['skse64_loader.exe', 'Data/SKSE'],
      enabled: true,
      source_url: undefined,
      provenance: 'detected',
    });
  });

  it('isHttpUrl guard', () => {
    expect(isHttpUrl('https://example.com/mod')).toBe(true);
    expect(isHttpUrl('  HTTP://example.com  ')).toBe(true);
    expect(isHttpUrl('ftp://example.com')).toBe(false);
    expect(isHttpUrl('example.com')).toBe(false);
    expect(isHttpUrl('')).toBe(false);
  });
});
