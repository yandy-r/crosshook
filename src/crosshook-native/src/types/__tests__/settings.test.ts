import { describe, expect, it } from 'vitest';
import { DEFAULT_APP_SETTINGS, toSettingsSaveRequest } from '../settings';

describe('settings type defaults', () => {
  it('defaults umu database lookup to disabled and includes it in save payloads', () => {
    expect(DEFAULT_APP_SETTINGS.umu_database_lookup).toBe('disabled');
    expect(toSettingsSaveRequest(DEFAULT_APP_SETTINGS).umu_database_lookup).toBe('disabled');
  });
});
