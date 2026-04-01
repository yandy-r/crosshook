import { useId, type ChangeEvent } from 'react';
import type { GamescopeConfig, GamescopeFilter } from '../types/profile';
import { CollapsibleSection } from './ui/CollapsibleSection';
import { ThemedSelect } from './ui/ThemedSelect';

export interface GamescopeConfigPanelProps {
  config: GamescopeConfig;
  onChange: (config: GamescopeConfig) => void;
  isInsideGamescopeSession: boolean;
}

const UPSCALE_FILTER_OPTIONS: Array<{ value: string; label: string }> = [
  { value: '', label: 'None' },
  { value: 'fsr', label: 'FSR' },
  { value: 'nis', label: 'NIS' },
  { value: 'linear', label: 'Linear' },
  { value: 'nearest', label: 'Nearest' },
  { value: 'pixel', label: 'Pixel' },
];

function parseOptionalInt(value: string): number | undefined {
  if (value === '') return undefined;
  const parsed = parseInt(value, 10);
  return isNaN(parsed) ? undefined : parsed;
}

export function GamescopeConfigPanel({ config, onChange, isInsideGamescopeSession }: GamescopeConfigPanelProps) {
  const id = useId();
  const isDisabled = !config.enabled;
  const showSessionWarning = isInsideGamescopeSession && config.enabled;
  const isFsr = config.upscale_filter === 'fsr';

  function patch(partial: Partial<GamescopeConfig>): void {
    onChange({ ...config, ...partial });
  }

  return (
    <div style={{ display: 'grid', gap: 16 }}>
      {/* Enable toggle */}
      <label htmlFor={`${id}-enable`} style={{ display: 'flex', alignItems: 'center', gap: 12, cursor: 'pointer' }}>
        <input
          id={`${id}-enable`}
          type="checkbox"
          checked={config.enabled}
          onChange={(e: ChangeEvent<HTMLInputElement>) => patch({ enabled: e.target.checked })}
          style={{ width: 20, height: 20, accentColor: 'var(--crosshook-color-accent-strong)' }}
        />
        <span style={{ color: 'var(--crosshook-color-text)', fontWeight: 700 }}>
          Enable gamescope compositor wrapper
        </span>
      </label>

      {/* Session warning banner */}
      {showSessionWarning ? (
        <div className="crosshook-warning-banner" role="alert">
          Running inside an existing gamescope session. Gamescope will be auto-skipped at launch unless &ldquo;Allow
          nested&rdquo; is enabled below.
        </div>
      ) : null}

      {/* Body — disabled when gamescope is off */}
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
        {/* Resolution */}
        <section style={{ display: 'grid', gap: 10 }}>
          <div className="crosshook-install-section-title">Resolution</div>
          <div className="crosshook-install-grid">
            <div className="crosshook-field">
              <span className="crosshook-label">Internal Resolution</span>
              <p className="crosshook-help-text" style={{ margin: 0 }}>
                Game renders at
              </p>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr auto 1fr', gap: 8, alignItems: 'center' }}>
                <input
                  id={`${id}-iw`}
                  type="number"
                  className="crosshook-input"
                  value={config.internal_width ?? ''}
                  placeholder="auto"
                  min={1}
                  disabled={isDisabled}
                  onChange={(e) => patch({ internal_width: parseOptionalInt(e.target.value) })}
                />
                <span style={{ color: 'var(--crosshook-color-text-subtle)', fontWeight: 600 }}>&times;</span>
                <input
                  id={`${id}-ih`}
                  type="number"
                  className="crosshook-input"
                  value={config.internal_height ?? ''}
                  placeholder="auto"
                  min={1}
                  disabled={isDisabled}
                  onChange={(e) => patch({ internal_height: parseOptionalInt(e.target.value) })}
                />
              </div>
            </div>

            <div className="crosshook-field">
              <span className="crosshook-label">Output Resolution</span>
              <p className="crosshook-help-text" style={{ margin: 0 }}>
                Display output
              </p>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr auto 1fr', gap: 8, alignItems: 'center' }}>
                <input
                  id={`${id}-ow`}
                  type="number"
                  className="crosshook-input"
                  value={config.output_width ?? ''}
                  placeholder="auto"
                  min={1}
                  disabled={isDisabled}
                  onChange={(e) => patch({ output_width: parseOptionalInt(e.target.value) })}
                />
                <span style={{ color: 'var(--crosshook-color-text-subtle)', fontWeight: 600 }}>&times;</span>
                <input
                  id={`${id}-oh`}
                  type="number"
                  className="crosshook-input"
                  value={config.output_height ?? ''}
                  placeholder="auto"
                  min={1}
                  disabled={isDisabled}
                  onChange={(e) => patch({ output_height: parseOptionalInt(e.target.value) })}
                />
              </div>
            </div>
          </div>
        </section>

        {/* Performance */}
        <section style={{ display: 'grid', gap: 10 }}>
          <div className="crosshook-install-section-title">Performance</div>
          <div className="crosshook-install-grid">
            <div className="crosshook-field">
              <label className="crosshook-label" htmlFor={`${id}-fps`}>
                Frame Rate Limit
              </label>
              <input
                id={`${id}-fps`}
                type="number"
                className="crosshook-input"
                value={config.frame_rate_limit ?? ''}
                placeholder="unlimited"
                min={1}
                disabled={isDisabled}
                onChange={(e) => patch({ frame_rate_limit: parseOptionalInt(e.target.value) })}
              />
            </div>

            <div className="crosshook-field">
              <label className="crosshook-label" htmlFor={`${id}-filter`}>
                Upscale Filter
              </label>
              <ThemedSelect
                id={`${id}-filter`}
                value={config.upscale_filter ?? ''}
                onValueChange={(value) =>
                  patch({
                    upscale_filter: value === '' ? undefined : (value as GamescopeFilter),
                    fsr_sharpness: value !== 'fsr' ? undefined : config.fsr_sharpness,
                  })
                }
                options={UPSCALE_FILTER_OPTIONS}
                placeholder="None"
              />
            </div>

            <div className="crosshook-field">
              <label className="crosshook-label" htmlFor={`${id}-fsr`} style={{ opacity: isFsr ? 1 : 0.5 }}>
                FSR Sharpness
                {!isFsr ? <span style={{ fontWeight: 400, marginLeft: 6 }}>(FSR filter only)</span> : null}
              </label>
              <input
                id={`${id}-fsr`}
                type="number"
                className="crosshook-input"
                value={config.fsr_sharpness ?? ''}
                placeholder="0-20"
                min={0}
                max={20}
                disabled={isDisabled || !isFsr}
                onChange={(e) => patch({ fsr_sharpness: parseOptionalInt(e.target.value) })}
              />
              <p className="crosshook-help-text">Higher values produce a sharper image. Range 0-20.</p>
            </div>
          </div>
        </section>

        {/* Display Flags */}
        <section style={{ display: 'grid', gap: 10 }}>
          <div className="crosshook-install-section-title">Display Flags</div>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 16 }}>
            <CheckboxFlag
              id={`${id}-fs`}
              label="Fullscreen"
              hint="-f"
              checked={config.fullscreen}
              disabled={isDisabled}
              onChange={(v) => patch({ fullscreen: v, borderless: v ? false : config.borderless })}
            />
            <CheckboxFlag
              id={`${id}-bl`}
              label="Borderless"
              hint="-b"
              checked={config.borderless}
              disabled={isDisabled}
              onChange={(v) => patch({ borderless: v, fullscreen: v ? false : config.fullscreen })}
            />
            <CheckboxFlag
              id={`${id}-gc`}
              label="Grab cursor"
              checked={config.grab_cursor}
              disabled={isDisabled}
              onChange={(v) => patch({ grab_cursor: v })}
            />
            <CheckboxFlag
              id={`${id}-fgc`}
              label="Force grab cursor"
              checked={config.force_grab_cursor}
              disabled={isDisabled}
              onChange={(v) => patch({ force_grab_cursor: v })}
            />
          </div>
        </section>

        {/* HDR */}
        <section style={{ display: 'grid', gap: 10 }}>
          <div className="crosshook-install-section-title">HDR</div>
          <CheckboxFlag
            id={`${id}-hdr`}
            label="Enable HDR"
            checked={config.hdr_enabled}
            disabled={isDisabled}
            onChange={(v) => patch({ hdr_enabled: v })}
          />
        </section>

        {/* Advanced */}
        <CollapsibleSection title="Advanced" defaultOpen={config.allow_nested || config.extra_args.length > 0}>
          <div style={{ display: 'grid', gap: 14, paddingTop: 4 }}>
            <div>
              <CheckboxFlag
                id={`${id}-nested`}
                label="Allow nested sessions"
                checked={config.allow_nested}
                disabled={isDisabled}
                onChange={(v) => patch({ allow_nested: v })}
              />
              <p className="crosshook-help-text" style={{ marginTop: 6, paddingLeft: 32 }}>
                Launch gamescope even when already inside a gamescope session (e.g. Steam Deck Game Mode).
              </p>
            </div>

            <div className="crosshook-field">
              <label className="crosshook-label" htmlFor={`${id}-extra`}>
                Extra arguments
              </label>
              <input
                id={`${id}-extra`}
                type="text"
                className="crosshook-input"
                value={config.extra_args.join(' ')}
                placeholder="e.g. --hdr-itm-enable --expose-wayland"
                disabled={isDisabled}
                onChange={(e: ChangeEvent<HTMLInputElement>) => {
                  const raw = e.target.value;
                  const args = raw === '' ? [] : raw.split(' ').filter((a) => a.length > 0);
                  patch({ extra_args: args });
                }}
              />
              <p className="crosshook-help-text">Space-separated extra CLI flags passed directly to gamescope.</p>
            </div>
          </div>
        </CollapsibleSection>
      </div>
    </div>
  );
}

function CheckboxFlag({
  id,
  label,
  hint,
  checked,
  disabled,
  onChange,
}: {
  id: string;
  label: string;
  hint?: string;
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
      <span style={{ color: 'var(--crosshook-color-text)', fontWeight: 600 }}>
        {label}
        {hint ? (
          <code
            style={{
              marginLeft: 6,
              fontSize: '0.82em',
              padding: '2px 6px',
              borderRadius: 4,
              background: 'rgba(255, 255, 255, 0.06)',
              color: 'var(--crosshook-color-text-muted)',
            }}
          >
            {hint}
          </code>
        ) : null}
      </span>
    </label>
  );
}

export default GamescopeConfigPanel;
