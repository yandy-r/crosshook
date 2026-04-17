import type { SVGProps } from 'react';

/* ── Per-route identity icons ──────────────────────────────────────────────
 * Used by RouteBanner as the per-route identity icon. Mirrors the sidebar
 * brand-art treatment: 64×64 viewBox, currentColor strokes, opacity vocabulary
 * 0.15–0.5. Each route keeps a unique illustration.
 */

const SVG_DEFAULTS: SVGProps<SVGSVGElement> = {
  viewBox: '0 0 64 64',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.5,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
};

export function LibraryArt() {
  return (
    <svg {...SVG_DEFAULTS} aria-hidden="true">
      {/* Shelf line */}
      <line x1="10" y1="50" x2="54" y2="50" opacity={0.3} />
      {/* Book spines */}
      <rect x="12" y="18" width="6" height="32" rx="1" opacity={0.4} />
      <rect x="20" y="14" width="6" height="36" rx="1" opacity={0.35} />
      <rect x="28" y="20" width="6" height="30" rx="1" opacity={0.3} />
      <rect x="36" y="16" width="6" height="34" rx="1" opacity={0.4} />
      <rect x="44" y="22" width="6" height="28" rx="1" opacity={0.3} />
      {/* Accent dot */}
      <circle cx="32" cy="10" r="2" fill="currentColor" opacity={0.25} stroke="none" />
    </svg>
  );
}

export function ProfilesArt() {
  return (
    <svg {...SVG_DEFAULTS} aria-hidden="true">
      {/* Layered cards */}
      <rect x="14" y="16" width="40" height="40" rx="6" opacity={0.15} />
      <rect x="10" y="12" width="40" height="40" rx="6" opacity={0.4} />
      {/* Avatar head */}
      <circle cx="30" cy="26" r="6" opacity={0.4} />
      {/* Avatar shoulders */}
      <path d="M20 42 a10 10 0 0 1 20 0" opacity={0.4} />
      {/* Accent dot */}
      <circle cx="50" cy="48" r="2" fill="currentColor" opacity={0.25} stroke="none" />
    </svg>
  );
}

export function LaunchArt() {
  return (
    <svg {...SVG_DEFAULTS} aria-hidden="true">
      {/* Outer halo ring */}
      <circle cx="34" cy="32" r="22" opacity={0.18} />
      {/* Play triangle */}
      <path d="M24 16 L50 32 L24 48 Z" opacity={0.4} />
      {/* Motion lines on left */}
      <line x1="8" y1="22" x2="16" y2="22" opacity={0.3} />
      <line x1="6" y1="32" x2="16" y2="32" opacity={0.35} />
      <line x1="8" y1="42" x2="16" y2="42" opacity={0.3} />
      {/* Center accent */}
      <circle cx="34" cy="32" r="2" fill="currentColor" opacity={0.25} stroke="none" />
    </svg>
  );
}

export function InstallArt() {
  return (
    <svg {...SVG_DEFAULTS} aria-hidden="true">
      {/* Card receiving the file */}
      <rect x="12" y="32" width="40" height="22" rx="4" opacity={0.35} />
      {/* Download arrow shaft */}
      <line x1="32" y1="8" x2="32" y2="34" strokeWidth={2} opacity={0.45} />
      {/* Arrow head */}
      <path d="M24 26 L32 36 L40 26" strokeWidth={2} opacity={0.45} />
      {/* Progress bar (track + filled) */}
      <rect x="18" y="44" width="28" height="3" rx="1.5" opacity={0.2} />
      <rect x="18" y="44" width="16" height="3" rx="1.5" fill="currentColor" opacity={0.3} stroke="none" />
      {/* Accent dot */}
      <circle cx="48" cy="14" r="2" fill="currentColor" opacity={0.2} stroke="none" />
    </svg>
  );
}

export function CommunityArt() {
  return (
    <svg {...SVG_DEFAULTS} aria-hidden="true">
      {/* Globe */}
      <circle cx="32" cy="32" r="20" opacity={0.4} />
      <ellipse cx="32" cy="32" rx="8" ry="20" opacity={0.22} />
      <line x1="12" y1="32" x2="52" y2="32" opacity={0.22} />
      <ellipse cx="32" cy="22" rx="18" ry="4" opacity={0.18} />
      <ellipse cx="32" cy="42" rx="18" ry="4" opacity={0.18} />
      {/* Center accent */}
      <circle cx="32" cy="32" r="2" fill="currentColor" opacity={0.25} stroke="none" />
    </svg>
  );
}

export function DiscoverArt() {
  return (
    <svg {...SVG_DEFAULTS} aria-hidden="true">
      {/* Magnifier lens */}
      <circle cx="26" cy="26" r="16" opacity={0.4} />
      <circle cx="26" cy="26" r="8" opacity={0.18} />
      {/* Crosshair inside lens */}
      <line x1="18" y1="26" x2="34" y2="26" opacity={0.25} />
      <line x1="26" y1="18" x2="26" y2="34" opacity={0.25} />
      {/* Handle */}
      <line x1="38" y1="38" x2="52" y2="52" strokeWidth={3} opacity={0.45} />
      {/* Center accent */}
      <circle cx="26" cy="26" r="1.5" fill="currentColor" opacity={0.25} stroke="none" />
    </svg>
  );
}

export function CompatibilityArt() {
  return (
    <svg {...SVG_DEFAULTS} aria-hidden="true">
      {/* Shield outer */}
      <path d="M32 8 L48 14 L48 32 Q48 46 32 56 Q16 46 16 32 L16 14 Z" opacity={0.4} />
      {/* Shield inner */}
      <path d="M32 14 L42 18 L42 32 Q42 42 32 50 Q22 42 22 32 L22 18 Z" opacity={0.18} />
      {/* Check mark */}
      <path d="M24 31 L30 37 L42 24" strokeWidth={2} opacity={0.5} />
      {/* Accent */}
      <circle cx="32" cy="32" r="1.5" fill="currentColor" opacity={0.2} stroke="none" />
    </svg>
  );
}

export function SettingsArt() {
  return (
    <svg {...SVG_DEFAULTS} aria-hidden="true">
      {/* Gear body */}
      <circle cx="32" cy="32" r="14" opacity={0.4} />
      <circle cx="32" cy="32" r="6" opacity={0.3} />
      {/* Cardinal teeth */}
      <line x1="32" y1="8" x2="32" y2="14" strokeWidth={3} opacity={0.35} strokeLinecap="round" />
      <line x1="32" y1="50" x2="32" y2="56" strokeWidth={3} opacity={0.35} strokeLinecap="round" />
      <line x1="8" y1="32" x2="14" y2="32" strokeWidth={3} opacity={0.35} strokeLinecap="round" />
      <line x1="50" y1="32" x2="56" y2="32" strokeWidth={3} opacity={0.35} strokeLinecap="round" />
      {/* Diagonal teeth */}
      <line x1="14" y1="14" x2="18" y2="18" strokeWidth={3} opacity={0.3} strokeLinecap="round" />
      <line x1="46" y1="46" x2="50" y2="50" strokeWidth={3} opacity={0.3} strokeLinecap="round" />
      <line x1="46" y1="18" x2="50" y2="14" strokeWidth={3} opacity={0.3} strokeLinecap="round" />
      <line x1="14" y1="50" x2="18" y2="46" strokeWidth={3} opacity={0.3} strokeLinecap="round" />
      {/* Center accent */}
      <circle cx="32" cy="32" r="2" fill="currentColor" opacity={0.25} stroke="none" />
    </svg>
  );
}

export function HealthDashboardArt() {
  return (
    <svg {...SVG_DEFAULTS} aria-hidden="true">
      {/* Dashboard card */}
      <rect x="8" y="14" width="48" height="36" rx="4" opacity={0.4} />
      {/* Top stat pills */}
      <rect x="12" y="18" width="11" height="6" rx="2" opacity={0.28} />
      <rect x="25" y="18" width="11" height="6" rx="2" opacity={0.22} />
      <rect x="38" y="18" width="14" height="6" rx="2" opacity={0.18} />
      {/* Heartbeat / chart line */}
      <path d="M12 38 L20 38 L24 28 L28 46 L32 30 L36 38 L52 38" strokeWidth={2} opacity={0.5} />
      {/* Accent dots */}
      <circle cx="17" cy="21" r="1.5" fill="currentColor" opacity={0.28} stroke="none" />
      <circle cx="56" cy="56" r="1.5" fill="currentColor" opacity={0.18} stroke="none" />
    </svg>
  );
}

export function HostToolsArt() {
  return (
    <svg {...SVG_DEFAULTS} aria-hidden="true">
      {/* Toolbox body */}
      <rect x="10" y="22" width="44" height="30" rx="3" opacity={0.4} />
      {/* Toolbox lid + handle */}
      <path d="M22 22v-4a4 4 0 0 1 4-4h12a4 4 0 0 1 4 4v4" opacity={0.32} />
      <rect x="28" y="10" width="8" height="6" rx="1.5" opacity={0.22} />
      {/* Inner divider */}
      <line x1="10" y1="34" x2="54" y2="34" opacity={0.3} />
      {/* Wrench */}
      <path
        d="M16 42a4 4 0 0 0 5.5 3.7l4.7 4.7a1.6 1.6 0 0 0 2.3-2.3l-4.7-4.7A4 4 0 0 0 16 42z"
        opacity={0.45}
        strokeWidth={1.5}
      />
      {/* Screwdriver */}
      <path d="M40 38 L48 46 M44 36 L46 38" strokeWidth={2.4} opacity={0.45} strokeLinecap="round" />
      {/* Status dot (capability healthy) */}
      <circle cx="50" cy="42" r="2" fill="currentColor" stroke="none" opacity={0.5} />
    </svg>
  );
}

export function ProtonManagerArt() {
  return (
    <svg {...SVG_DEFAULTS} aria-hidden="true">
      {/* Download tray */}
      <rect x="10" y="38" width="44" height="14" rx="3" opacity={0.35} />
      {/* Download arrow shaft */}
      <line x1="32" y1="8" x2="32" y2="36" strokeWidth={2} opacity={0.45} />
      {/* Arrow head */}
      <path d="M23 27 L32 38 L41 27" strokeWidth={2} opacity={0.45} />
      {/* Progress bar track + filled */}
      <rect x="14" y="44" width="36" height="3" rx="1.5" opacity={0.18} />
      <rect x="14" y="44" width="22" height="3" rx="1.5" fill="currentColor" opacity={0.32} stroke="none" />
      {/* Proton "P" hint — version chip */}
      <rect x="12" y="14" width="12" height="10" rx="2" opacity={0.22} />
      <path d="M15 17 h4 a2 2 0 0 1 0 4 h-4" opacity={0.4} strokeWidth={1.5} />
      {/* Accent dot */}
      <circle cx="50" cy="14" r="2" fill="currentColor" opacity={0.22} stroke="none" />
    </svg>
  );
}
