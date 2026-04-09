import { callCommand } from '@/lib/ipc';
import { convertFileSrc } from '@/lib/plugin-stubs/convertFileSrc';

import type { GameProfile, LaunchMethod } from '../../types';
import { FieldRow } from '../ProfileFormSections';
import { chooseFile } from '../../utils/dialog';
import { resolveArtAppId } from '../../utils/art';

export interface MediaSectionProps {
  profile: GameProfile;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  launchMethod: LaunchMethod;
}

type ArtSlotType = 'cover' | 'portrait' | 'background';

type ArtSourceBadge = 'Custom' | 'Auto' | 'Not Set';

function resolveSourceBadge(customPath: string | undefined, effectiveAppId: string): ArtSourceBadge {
  if (customPath?.trim()) return 'Custom';
  if (effectiveAppId.trim()) return 'Auto';
  return 'Not Set';
}

const SLOT_CONFIG: Record<ArtSlotType, { label: string; hint: string }> = {
  cover: {
    label: 'Cover',
    hint: 'Full-width backdrop behind profile tabs (460×215 / ~2.14:1).',
  },
  portrait: {
    label: 'Portrait',
    hint: 'Library grid card (600×900 / 2:3).',
  },
  background: {
    label: 'Background',
    hint: 'Wide hero banner (3840×1240 / ~3:1).',
  },
};

const FIELD_KEY_MAP: Record<
  ArtSlotType,
  'custom_cover_art_path' | 'custom_portrait_art_path' | 'custom_background_art_path'
> = {
  cover: 'custom_cover_art_path',
  portrait: 'custom_portrait_art_path',
  background: 'custom_background_art_path',
};

export function MediaSection({ profile, onUpdateProfile, launchMethod }: MediaSectionProps) {
  const effectiveAppId = resolveArtAppId(profile);

  const handleBrowse = async (artType: ArtSlotType) => {
    const fieldKey = FIELD_KEY_MAP[artType];
    const path = await chooseFile(`Select ${SLOT_CONFIG[artType].label} Art`, [
      { name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] },
    ]);
    if (!path) return;
    try {
      const imported = await callCommand<string>('import_custom_art', { sourcePath: path, artType });
      onUpdateProfile((c) => ({ ...c, game: { ...c.game, [fieldKey]: imported } }));
    } catch (err) {
      console.error(`Failed to import ${artType} art`, err);
    }
  };

  const handleClear = (artType: ArtSlotType) => {
    onUpdateProfile((c) => ({ ...c, game: { ...c.game, [FIELD_KEY_MAP[artType]]: undefined } }));
  };

  const handleChange = (artType: ArtSlotType, value: string) => {
    onUpdateProfile((c) => ({ ...c, game: { ...c.game, [FIELD_KEY_MAP[artType]]: value || undefined } }));
  };

  const slots: ArtSlotType[] = ['cover', 'portrait', 'background'];

  return (
    <>
      <div className="crosshook-install-section-title">Game Art</div>
      <p className="crosshook-help-text" style={{ marginTop: 0 }}>
        Custom art overrides auto-downloaded images. Without a Steam App ID, custom art is the only source.
      </p>

      <div className="crosshook-install-grid">
        {slots.map((slotType) => {
          const cfg = SLOT_CONFIG[slotType];
          const fieldKey = FIELD_KEY_MAP[slotType];
          const customPath = profile.game[fieldKey];
          const badge = resolveSourceBadge(customPath, effectiveAppId);
          const previewSrc = customPath?.trim() ? convertFileSrc(customPath.trim()) : null;

          return (
            <div key={slotType} className="crosshook-media-slot">
              <div className="crosshook-media-slot__header">
                <span className="crosshook-label">{cfg.label}</span>
                <span
                  className={`crosshook-media-slot__badge crosshook-media-slot__badge--${badge.toLowerCase().replace(' ', '-')}`}
                >
                  {badge}
                </span>
              </div>

              <div className="crosshook-media-slot__preview">
                {previewSrc ? (
                  <img src={previewSrc} alt={`${cfg.label} preview`} className="crosshook-media-slot__img" />
                ) : (
                  <div className="crosshook-media-slot__placeholder" />
                )}
              </div>

              <FieldRow
                label=""
                value={customPath ?? ''}
                onChange={(v) => handleChange(slotType, v)}
                placeholder={`/path/to/${slotType}-art.png`}
                browseLabel="Browse"
                onBrowse={() => handleBrowse(slotType)}
                helperText={cfg.hint}
              />

              {badge === 'Custom' && (
                <button
                  type="button"
                  className="crosshook-button crosshook-button--secondary"
                  onClick={() => handleClear(slotType)}
                  style={{ alignSelf: 'flex-start', marginTop: 4 }}
                >
                  Clear
                </button>
              )}
            </div>
          );
        })}
      </div>

      {launchMethod !== 'native' && (
        <>
          <div className="crosshook-install-section-title" style={{ marginTop: 24 }}>
            Launcher Icon
          </div>
          <div className="crosshook-install-grid">
            <FieldRow
              label="Icon Path"
              value={profile.steam.launcher.icon_path}
              onChange={(value) =>
                onUpdateProfile((c) => ({
                  ...c,
                  steam: { ...c.steam, launcher: { ...c.steam.launcher, icon_path: value } },
                }))
              }
              placeholder="/path/to/icon.png"
              browseLabel="Browse"
              onBrowse={async () => {
                const path = await chooseFile('Select Launcher Icon', [
                  { name: 'Images', extensions: ['png', 'jpg', 'jpeg'] },
                ]);
                if (path) {
                  onUpdateProfile((c) => ({
                    ...c,
                    steam: { ...c.steam, launcher: { ...c.steam.launcher, icon_path: path } },
                  }));
                }
              }}
            />
          </div>
        </>
      )}
    </>
  );
}

export default MediaSection;
