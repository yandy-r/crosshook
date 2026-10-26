import { describe, expect, it } from 'vitest';
import { DEFAULT_APP_SETTINGS, DEFAULT_UI_SETTINGS, toSettingsSaveRequest } from '../settings';

describe('settings type defaults', () => {
  it('defaults umu database lookup to disabled and includes it in save payloads', () => {
    expect(DEFAULT_APP_SETTINGS.umu_database_lookup).toBe('disabled');
    expect(toSettingsSaveRequest(DEFAULT_APP_SETTINGS).umu_database_lookup).toBe('disabled');
  });

  it('defaults the [ui] table to DEFAULT_UI_SETTINGS', () => {
    expect(DEFAULT_APP_SETTINGS.ui).toEqual(DEFAULT_UI_SETTINGS);
    // Mutating a seed cloned from the default must never leak into the shared default.
    const seed = structuredClone(DEFAULT_APP_SETTINGS);
    seed.ui.palette_recent.push('mutated');
    expect(DEFAULT_UI_SETTINGS.palette_recent).toEqual([]);
    expect(DEFAULT_APP_SETTINGS.ui.palette_recent).toEqual([]);
  });

  it('never emits the [ui] table in save payloads', () => {
    const request = toSettingsSaveRequest({
      ...DEFAULT_APP_SETTINGS,
      ui: { ...DEFAULT_UI_SETTINGS, library_view_mode: 'list' },
    });
    expect('ui' in request).toBe(false);
  });
});
