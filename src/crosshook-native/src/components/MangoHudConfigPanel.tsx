import { type ChangeEvent, useId } from 'react';
import { open as openShell } from '@/lib/plugin-stubs/shell';
import { useCapabilityGate } from '../hooks/useCapabilityGate';
import { type MangoHudPreset, useMangoHudPresets } from '../hooks/useMangoHudPresets';
import type { MangoHudConfig, MangoHudPosition } from '../types/profile';
import { CollapsibleSection } from './ui/CollapsibleSection';
import { ThemedSelect } from './ui/ThemedSelect';

export interface MangoHudConfigPanelProps {
  config: MangoHudConfig;
  onChange: (config: MangoHudConfig) => void;
  disabled?: boolean;
  showMangoHudOverlayEnabled?: boolean;
  launchMethod?: string;
}

const POSITION_OPTIONS: Array<{ value: string; label: string }> = [
  { value: 'top-left', label: 'Top Left' },
  { value: 'top-center', label: 'Top Center' },
  { value: 'top-right', label: 'Top Right' },
  { value: 'bottom-left', label: 'Bottom Left' },
  { value: 'bottom-center', label: 'Bottom Center' },
  { value: 'bottom-right', label: 'Bottom Right' },
];

const VALID_MANGOHUD_POSITIONS: readonly MangoHudPosition[] = [
  'top-left',
  'top-center',
  'top-right',
  'bottom-left',
  'bottom-center',
  'bottom-right',
];

function parseMangoHudPosition(value: string | undefined): MangoHudPosition | undefined {
  if (!value) return undefined;
  return VALID_MANGOHUD_POSITIONS.includes(value as MangoHudPosition) ? (value as MangoHudPosition) : undefined;
}

function parseOptionalInt(value: string): number | undefined {
  if (value === '') return undefined;
  const parsed = parseInt(value, 10);
  return Number.isNaN(parsed) ? undefined : parsed;
}

function detectActivePreset(config: MangoHudConfig, presets: MangoHudPreset[]): string {
  for (const preset of presets) {
    if (
      config.fps_limit === preset.fps_limit &&
      config.gpu_stats === preset.gpu_stats &&
      config.cpu_stats === preset.cpu_stats &&
      config.ram === preset.ram &&
      config.frametime === preset.frametime &&
      config.battery === preset.battery &&
      config.watt === preset.watt &&
      parseMangoHudPosition(config.position) === parseMangoHudPosition(preset.position)
    ) {
      return preset.id;
    }
  }
  return '';
}

export function MangoHudConfigPanel({
  config,
  onChange,
  disabled = false,
  showMangoHudOverlayEnabled,
  launchMethod,
}: MangoHudConfigPanelProps) {
  const id = useId();
  const mangohudGate = useCapabilityGate('mangohud');
  const isCapabilityUnavailable = mangohudGate.state === 'unavailable';
  const isDisabled = disabled || !config.enabled || isCapabilityUnavailable;
  const showActivationHint = config.enabled && showMangoHudOverlayEnabled === false;
  const showMethodNote = launchMethod === 'steam_applaunch';
  const { presets } = useMangoHudPresets();

  function patch(partial: Partial<MangoHudConfig>): void {
    onChange({ ...config, ...partial });
  }

  function applyPreset(preset: MangoHudPreset): void {
    onChange({
      ...config,
      enabled: config.enabled,
      fps_limit: preset.fps_limit,
      gpu_stats: preset.gpu_stats,
      cpu_stats: preset.cpu_stats,
      ram: preset.ram,
      frametime: preset.frametime,
      battery: preset.battery,
      watt: preset.watt,
      position: parseMangoHudPosition(preset.position),
    });
  }

  const activePresetId = detectActivePreset(config, presets);
  const activePreset = presets.find((p) => p.id === activePresetId) ?? null;

  const presetOptions = [{ value: '', label: 'Custom' }, ...presets.map((p) => ({ value: p.id, label: p.label }))];

  return (
    <div style={{ display: 'grid', gap: 16 }}>
      {/* Enable toggle */}
      <label
        htmlFor={`${id}-enable`}
        style={{ display: 'flex', alignItems: 'center', gap: 12, cursor: disabled ? 'default' : 'pointer' }}
      >
        <input
          id={`${id}-enable`}
          type="checkbox"
          checked={config.enabled}
          disabled={disabled}
          onChange={(e: ChangeEvent<HTMLInputElement>) => patch({ enabled: e.target.checked })}
          style={{ width: 20, height: 20, accentColor: 'var(--crosshook-color-accent-strong)' }}
        />
        <span style={{ color: 'var(--crosshook-color-text)', fontWeight: 700 }}>
          Enable per-profile MangoHud config
        </span>
      </label>

      {mangohudGate.rationale ? (
        <div
          className={isCapabilityUnavailable ? 'crosshook-warning-banner' : 'crosshook-info-banner'}
          role={isCapabilityUnavailable ? 'alert' : 'note'}
        >
          <div style={{ display: 'grid', gap: 8 }}>
            <span>{mangohudGate.rationale}</span>
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              {mangohudGate.onCopyCommand ? (
                <button
                  type="button"
                  className="crosshook-button crosshook-button--ghost crosshook-button--small"
                  onClick={() => {
                    void mangohudGate.onCopyCommand?.();
                  }}
                >
                  Copy install command
                </button>
              ) : null}
              {mangohudGate.docsUrl ? (
                <button
                  type="button"
                  className="crosshook-button crosshook-button--ghost crosshook-button--small"
                  onClick={() => {
                    void openShell(mangohudGate.docsUrl ?? '');
                  }}
                >
                  Open docs
                </button>
              ) : null}
            </div>
          </div>
        </div>
      ) : null}

      {/* Activation hint */}
      {showActivationHint ? (
        <div className="crosshook-warning-banner" role="note">
          Enable &ldquo;MangoHud overlay&rdquo; in Launch Optimizations for this config to take effect.
        </div>
      ) : null}

      {/* Launch method note */}
      {showMethodNote ? (
        <div className="crosshook-warning-banner" role="note">
          Per-profile MangoHud config is only supported with the proton_run launch method.
        </div>
      ) : null}

      {/* Body — disabled when MangoHud config is off */}
      <div
        style={{
          display: 'grid',
          gap: 20,
          opacity: isDisabled ? 0.4 : undefined,
          pointerEvents: isDisabled ? 'none' : undefined,
          transition: 'opacity 220ms ease',
        }}
        aria-disabled={isDisabled}
      >
        {/* Preset selector */}
        {presets.length > 0 ? (
          <section style={{ display: 'grid', gap: 10 }}>
            <div className="crosshook-install-section-title">Preset</div>
            <div className="crosshook-field">
              <label className="crosshook-label" htmlFor={`${id}-preset`}>
                Quick Preset
              </label>
              <ThemedSelect
                id={`${id}-preset`}
                value={activePresetId}
                onValueChange={(value) => {
                  if (value === '') return;
                  const preset = presets.find((p) => p.id === value);
                  if (preset) applyPreset(preset);
                }}
                options={presetOptions}
                placeholder="Custom"
              />
              {activePreset ? <p className="crosshook-help-text">{activePreset.description}</p> : null}
            </div>
          </section>
        ) : null}

        {/* Overlay position and FPS limit */}
        <section style={{ display: 'grid', gap: 10 }}>
          <div className="crosshook-install-section-title">Display</div>
          <div className="crosshook-install-grid">
            <div className="crosshook-field">
              <label className="crosshook-label" htmlFor={`${id}-position`}>
                Position
              </label>
              <ThemedSelect
                id={`${id}-position`}
                value={config.position ?? ''}
                onValueChange={(value) => patch({ position: parseMangoHudPosition(value) })}
                options={[{ value: '', label: 'Default' }, ...POSITION_OPTIONS]}
                placeholder="Default"
              />
            </div>

            <div className="crosshook-field">
              <label className="crosshook-label" htmlFor={`${id}-fps-limit`}>
                FPS Limit
              </label>
              <input
                id={`${id}-fps-limit`}
                type="number"
                className="crosshook-input"
                value={config.fps_limit ?? ''}
                placeholder="no limit"
                min={0}
                disabled={isDisabled}
                onChange={(e) => patch({ fps_limit: parseOptionalInt(e.target.value) })}
              />
              <p className="crosshook-help-text">Set to 0 or leave empty for no FPS limit.</p>
            </div>
          </div>
        </section>

        {/* Stats toggles */}
        <CollapsibleSection
          title="Displayed Stats"
          defaultOpen={
            config.gpu_stats || config.cpu_stats || config.ram || config.frametime || config.battery || config.watt
          }
        >
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 16, paddingTop: 4 }}>
            <CheckboxFlag
              id={`${id}-gpu-stats`}
              label="GPU stats"
              checked={config.gpu_stats}
              disabled={isDisabled}
              onChange={(v) => patch({ gpu_stats: v })}
            />
            <CheckboxFlag
              id={`${id}-cpu-stats`}
              label="CPU stats"
              checked={config.cpu_stats}
              disabled={isDisabled}
              onChange={(v) => patch({ cpu_stats: v })}
            />
            <CheckboxFlag
              id={`${id}-ram`}
              label="RAM usage"
              checked={config.ram}
              disabled={isDisabled}
              onChange={(v) => patch({ ram: v })}
            />
            <CheckboxFlag
              id={`${id}-frametime`}
              label="Frame time"
              checked={config.frametime}
              disabled={isDisabled}
              onChange={(v) => patch({ frametime: v })}
            />
            <CheckboxFlag
              id={`${id}-battery`}
              label="Battery"
              checked={config.battery}
              disabled={isDisabled}
              onChange={(v) => patch({ battery: v })}
            />
            <CheckboxFlag
              id={`${id}-watt`}
              label="Power draw (watts)"
              checked={config.watt}
              disabled={isDisabled}
              onChange={(v) => patch({ watt: v })}
            />
          </div>
        </CollapsibleSection>
      </div>
    </div>
  );
}

function CheckboxFlag({
  id,
  label,
  checked,
  disabled,
  onChange,
}: {
  id: string;
  label: string;
  checked: boolean;
  disabled: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <label
      htmlFor={id}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 10,
        cursor: disabled ? 'default' : 'pointer',
        minHeight: 'var(--crosshook-touch-target-compact)',
      }}
    >
      <input
        id={id}
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e: ChangeEvent<HTMLInputElement>) => onChange(e.target.checked)}
        style={{ width: 20, height: 20, accentColor: 'var(--crosshook-color-accent-strong)', flex: '0 0 auto' }}
      />
      <span style={{ color: 'var(--crosshook-color-text)', fontWeight: 600 }}>{label}</span>
    </label>
  );
}

export default MangoHudConfigPanel;
