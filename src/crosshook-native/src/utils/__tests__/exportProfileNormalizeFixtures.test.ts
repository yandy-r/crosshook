/**
 * Golden parity fixtures for `crosshook_core::profile::normalize_profile`.
 *
 * Reads every `*.input.json` draft under
 * `crates/crosshook-core/tests/fixtures/profile_normalize/`, runs the real
 * Classic `normalizeProfileForSave(draft, {}, false)` (the exact pre-IPC save
 * normalizer), and compares against the committed `*.classic.normalized.json`.
 *
 * `scripts/export-ts-parity-fixtures.sh` runs this test with
 * `CROSSHOOK_WRITE_PARITY_FIXTURES=1` to rewrite the committed snapshots and
 * then regenerates `*.classic.toml` via `ProfileStore::save`.
 */
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { normalizeProfileForSave } from '@/hooks/profile/profileNormalize';
import { normalizeSerializedGameProfile, type SerializedGameProfile } from '@/types/profile';

const FIXTURE_DIR = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../../crates/crosshook-core/tests/fixtures/profile_normalize'
);
const WRITE = process.env.CROSSHOOK_WRITE_PARITY_FIXTURES === '1';
if (WRITE && process.env.CI) {
  throw new Error('CROSSHOOK_WRITE_PARITY_FIXTURES must not be set in CI: it would hide fixture drift.');
}

function listInputFixtures(): string[] {
  return readdirSync(FIXTURE_DIR)
    .filter((name) => name.endsWith('.input.json'))
    .sort();
}

describe('TS parity fixtures for crosshook-core profile normalization', () => {
  const inputs = listInputFixtures();

  it('discovers profile_normalize input fixtures', () => {
    expect(inputs.length).toBeGreaterThanOrEqual(8);
  });

  it.each(inputs)('%s matches committed Classic normalized output', (inputFile) => {
    const stem = inputFile.slice(0, -'.input.json'.length);
    const raw = JSON.parse(readFileSync(path.join(FIXTURE_DIR, inputFile), 'utf8')) as SerializedGameProfile;
    const normalized = normalizeProfileForSave(normalizeSerializedGameProfile(raw), {}, false);
    const outPath = path.join(FIXTURE_DIR, `${stem}.classic.normalized.json`);
    const text = `${JSON.stringify(normalized, null, 2)}\n`;

    if (WRITE) {
      writeFileSync(outPath, text);
      return;
    }

    expect(JSON.parse(readFileSync(outPath, 'utf8'))).toEqual(JSON.parse(text));
  });
});
