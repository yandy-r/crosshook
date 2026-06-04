import { describe, expect, it } from 'vitest';
import { createDefaultProfile } from '@/types/profile';
import { applyCreateSeed, type ProfileCreateSeed } from '../profileCreateSeed';

function blankProfile() {
  return createDefaultProfile();
}

describe('applyCreateSeed', () => {
  describe('field mapping', () => {
    it('maps gameName to game.name', () => {
      const result = applyCreateSeed(blankProfile(), { gameName: 'Synthetic Quest' });
      expect(result.game.name).toBe('Synthetic Quest');
    });

    it('maps executablePath to game.executable_path', () => {
      const result = applyCreateSeed(blankProfile(), {
        executablePath: '/games/sq/game.exe',
      });
      expect(result.game.executable_path).toBe('/games/sq/game.exe');
    });

    it('maps coverArtPath to game.custom_cover_art_path', () => {
      const result = applyCreateSeed(blankProfile(), { coverArtPath: '/art/cover.jpg' });
      expect(result.game.custom_cover_art_path).toBe('/art/cover.jpg');
    });

    it('maps portraitArtPath to game.custom_portrait_art_path', () => {
      const result = applyCreateSeed(blankProfile(), { portraitArtPath: '/art/portrait.jpg' });
      expect(result.game.custom_portrait_art_path).toBe('/art/portrait.jpg');
    });

    it('maps valid steamAppId to steam.app_id, sets steam.enabled, and sets runtime.steam_app_id', () => {
      const result = applyCreateSeed(blankProfile(), { steamAppId: '123456' });
      expect(result.steam.app_id).toBe('123456');
      expect(result.steam.enabled).toBe(true);
      expect(result.runtime.steam_app_id).toBe('123456');
    });

    it('does not touch suggestedName on the profile (it is a wizard-only field)', () => {
      const base = blankProfile();
      const result = applyCreateSeed(base, { suggestedName: 'My Name' });
      // Profile game.name should remain unchanged
      expect(result.game.name).toBe(base.game.name);
    });
  });

  describe('numeric steamAppId guard', () => {
    it('rejects non-numeric appId', () => {
      const base = blankProfile();
      const result = applyCreateSeed(base, { steamAppId: 'abc' });
      expect(result.steam.app_id).toBe(base.steam.app_id);
      expect(result.steam.enabled).toBe(base.steam.enabled);
      expect(result.runtime.steam_app_id).toBe(base.runtime.steam_app_id);
    });

    it('rejects mixed alphanumeric appId', () => {
      const base = blankProfile();
      const result = applyCreateSeed(base, { steamAppId: '123abc' });
      expect(result.steam.app_id).toBe(base.steam.app_id);
    });

    it('rejects appId longer than 12 digits', () => {
      const base = blankProfile();
      const thirteenDigits = '1234567890123'; // 13 chars
      const result = applyCreateSeed(base, { steamAppId: thirteenDigits });
      expect(result.steam.app_id).toBe(base.steam.app_id);
    });

    it('accepts a 12-digit appId at the boundary', () => {
      const result = applyCreateSeed(blankProfile(), { steamAppId: '123456789012' });
      expect(result.steam.app_id).toBe('123456789012');
      expect(result.steam.enabled).toBe(true);
    });

    it('accepts a single-digit appId', () => {
      const result = applyCreateSeed(blankProfile(), { steamAppId: '1' });
      expect(result.steam.app_id).toBe('1');
    });

    it('rejects an empty steamAppId string', () => {
      const base = blankProfile();
      const result = applyCreateSeed(base, { steamAppId: '' });
      expect(result.steam.app_id).toBe(base.steam.app_id);
    });
  });

  describe('empty seed no-op', () => {
    it('returns an equivalent profile when seed is empty', () => {
      const base = blankProfile();
      const result = applyCreateSeed(base, {});
      expect(result.game).toEqual(base.game);
      expect(result.steam).toEqual(base.steam);
      expect(result.runtime).toEqual(base.runtime);
    });

    it('returns an equivalent profile when all seed fields are undefined', () => {
      const seed: ProfileCreateSeed = {
        suggestedName: undefined,
        gameName: undefined,
        steamAppId: undefined,
        executablePath: undefined,
        coverArtPath: undefined,
        portraitArtPath: undefined,
      };
      const base = blankProfile();
      const result = applyCreateSeed(base, seed);
      expect(result.game).toEqual(base.game);
      expect(result.steam).toEqual(base.steam);
      expect(result.runtime).toEqual(base.runtime);
    });
  });

  describe('purity', () => {
    it('does not mutate the input profile', () => {
      const base = blankProfile();
      const frozen = JSON.parse(JSON.stringify(base)) as typeof base;
      applyCreateSeed(base, {
        gameName: 'Some Game',
        steamAppId: '999',
        executablePath: '/games/x.exe',
        coverArtPath: '/art/cover.jpg',
        portraitArtPath: '/art/portrait.jpg',
      });
      expect(base).toEqual(frozen);
    });

    it('returns a new object reference even for empty seed', () => {
      const base = blankProfile();
      const result = applyCreateSeed(base, {});
      expect(result).not.toBe(base);
    });
  });
});
