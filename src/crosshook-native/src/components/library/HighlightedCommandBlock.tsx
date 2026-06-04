import type { LaunchPreview, PreviewEnvVar } from '@/types/launch';

export interface HighlightedCommandBlockProps {
  preview: LaunchPreview;
  profileName?: string;
  className?: string;
}

type TokenTone = 'comment' | 'env-key' | 'value' | 'binary' | 'flag';

interface CommandToken {
  key: string;
  text: string;
  tone: TokenTone;
}

function tokenClass(tone: TokenTone): string {
  return `crosshook-hero-detail__command-token crosshook-hero-detail__command-token--${tone}`;
}

function customPreviewEnv(environment: PreviewEnvVar[] | null): PreviewEnvVar[] {
  return (environment ?? []).filter((envVar) => envVar.source === 'profile_custom');
}

type PushToken = (text: string, tone: TokenTone) => void;

const FLAG_TOKEN = /^-{1,2}[a-zA-Z0-9][\w\-=.]*$/;
const MAX_FLAG_TOKENS = 8;
const MAX_DISPLAY_FIELD_LEN = 64;

/** Keep preview comment suffixes readable and stable for React keys. */
function sanitizeDisplayField(value: string): string {
  const printable = value.replace(/[^\x20-\x7E]/g, '');
  return printable.length > MAX_DISPLAY_FIELD_LEN ? `${printable.slice(0, MAX_DISPLAY_FIELD_LEN)}…` : printable;
}

function isFlagToken(part: string): boolean {
  return part !== '--' && FLAG_TOKEN.test(part);
}

function pushEnvTokens(push: PushToken, environment: PreviewEnvVar[] | null) {
  for (const envVar of customPreviewEnv(environment)) {
    push(envVar.key, 'env-key');
    push('=', 'comment');
    push(JSON.stringify(envVar.value), 'value');
    push(' ', 'comment');
  }
}

function pushWordTokens(push: PushToken, words: string[], tone: TokenTone) {
  for (const word of words) {
    const trimmed = word.trim();
    if (!trimmed) {
      continue;
    }
    push(trimmed, tone);
    push(' ', 'comment');
  }
}

function commandTokens(preview: LaunchPreview): CommandToken[] {
  const tokens: CommandToken[] = [];
  let cursor = 0;
  const push: PushToken = (text, tone) => {
    cursor += text.length + tone.length + 1;
    tokens.push({ key: `${tone}-${cursor}`, text, tone });
  };

  push(`# ${sanitizeDisplayField(preview.resolved_method)} launch preview`, 'comment');
  if (preview.game_executable_name || preview.game_executable) {
    const gameLabel = sanitizeDisplayField(preview.game_executable_name || preview.game_executable);
    push(` for ${gameLabel}`, 'comment');
  }
  push('\n', 'comment');

  pushEnvTokens(push, preview.environment);
  pushWordTokens(push, preview.wrappers ?? [], 'binary');

  if (preview.proton_setup?.proton_executable) {
    push(preview.proton_setup.proton_executable, 'binary');
    push(' run ', 'flag');
  }

  push(preview.game_executable, 'binary');

  if (preview.effective_command) {
    const allFlags = preview.effective_command.split(/\s+/).filter(isFlagToken);
    const truncated = allFlags.length > MAX_FLAG_TOKENS;
    const flags = truncated ? allFlags.slice(0, MAX_FLAG_TOKENS) : allFlags;
    if (flags.length > 0) {
      push(' ', 'comment');
      pushWordTokens(push, flags, 'flag');
      if (truncated) {
        push('…', 'comment');
      }
    }
  }

  return tokens;
}

export function HighlightedCommandBlock({ preview, profileName, className }: HighlightedCommandBlockProps) {
  const classes = ['crosshook-hero-detail__highlighted-command', className].filter(Boolean).join(' ');
  const label = profileName ? `${profileName} launch command preview` : 'Launch command preview';
  const tokens = commandTokens(preview);

  return (
    <figure className="crosshook-hero-detail__command-figure" aria-label={label}>
      <pre className={classes}>
        <code>
          {tokens.map((token) => (
            <span key={token.key} className={tokenClass(token.tone)}>
              {token.text}
            </span>
          ))}
        </code>
      </pre>
    </figure>
  );
}

export default HighlightedCommandBlock;
