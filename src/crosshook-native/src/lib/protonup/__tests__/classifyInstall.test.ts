import { describe, expect, it } from 'vitest';
import { classifyInstallProvider, normalizeInstallToTag } from '../classifyInstall';

describe('normalizeInstallToTag', () => {
  it('strips the x86_64 suffix from GE-Proton 11-5+ directories', () => {
    expect(normalizeInstallToTag('GE-Proton11-5-x86_64', 'ge-proton')).toBe('GE-Proton11-5');
  });

  it('strips the suffix regardless of case', () => {
    expect(normalizeInstallToTag('GE-Proton11-5-X86_64', 'ge-proton')).toBe('GE-Proton11-5');
  });

  it('keeps older GE-Proton directory names unchanged', () => {
    expect(normalizeInstallToTag('GE-Proton9-21', 'ge-proton')).toBe('GE-Proton9-21');
  });

  it('still maps Proton-CachyOS directories to their tag', () => {
    expect(normalizeInstallToTag('proton-cachyos-10.0-20260410-slr-x86_64', 'proton-cachyos')).toBe(
      'cachyos-10.0-20260410-slr'
    );
  });

  it('classifies the new GE-Proton directory as ge-proton', () => {
    expect(classifyInstallProvider('GE-Proton11-5-x86_64')).toBe('ge-proton');
  });
});
