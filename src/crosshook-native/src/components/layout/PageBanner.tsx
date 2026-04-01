import type { SVGProps } from 'react';

/* ── Per-route decorative illustrations (used by route backdrop in ContentArea) ── */

const SVG_DEFAULTS: SVGProps<SVGSVGElement> = {
  viewBox: '0 0 200 120',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
};

export function ProfilesArt() {
  return (
    <svg {...SVG_DEFAULTS}>
      <rect x="60" y="20" width="80" height="80" rx="12" strokeWidth={1.2} opacity={0.5} />
      <circle cx="100" cy="48" r="14" opacity={0.45} />
      <path d="M80 78a20 20 0 0 1 40 0" opacity={0.4} />
      <rect x="48" y="28" width="80" height="80" rx="12" opacity={0.15} />
      <rect x="72" y="12" width="80" height="80" rx="12" opacity={0.1} />
      <circle cx="155" cy="30" r="3" fill="currentColor" opacity={0.12} stroke="none" />
      <circle cx="40" cy="90" r="4" fill="currentColor" opacity={0.1} stroke="none" />
      <circle cx="170" cy="85" r="2" fill="currentColor" opacity={0.15} stroke="none" />
    </svg>
  );
}

export function LaunchArt() {
  return (
    <svg {...SVG_DEFAULTS}>
      <path d="M65 25 L150 60 L65 95Z" strokeWidth={1.2} opacity={0.4} />
      <line x1="40" y1="45" x2="55" y2="45" opacity={0.25} />
      <line x1="35" y1="60" x2="55" y2="60" opacity={0.3} />
      <line x1="40" y1="75" x2="55" y2="75" opacity={0.25} />
      <path d="M60 40 Q45 60 60 80" opacity={0.15} strokeWidth={1.5} />
      <path d="M50 35 Q30 60 50 85" opacity={0.08} strokeWidth={2} />
      <circle cx="160" cy="35" r="2.5" fill="currentColor" opacity={0.15} stroke="none" />
      <circle cx="165" cy="80" r="1.5" fill="currentColor" opacity={0.12} stroke="none" />
      <circle cx="30" cy="30" r="3" fill="currentColor" opacity={0.08} stroke="none" />
    </svg>
  );
}

export function InstallArt() {
  return (
    <svg {...SVG_DEFAULTS}>
      <rect x="65" y="45" width="70" height="50" rx="10" strokeWidth={1.2} opacity={0.4} />
      <line x1="100" y1="10" x2="100" y2="55" strokeWidth={1.5} opacity={0.45} />
      <path d="M88 43 L100 58 L112 43" strokeWidth={1.5} opacity={0.45} />
      <path d="M65 55 Q100 40 135 55" opacity={0.2} />
      <rect x="75" y="78" width="50" height="4" rx="2" opacity={0.2} />
      <rect x="75" y="78" width="30" height="4" rx="2" fill="currentColor" opacity={0.15} stroke="none" />
      <circle cx="155" cy="25" r="2" fill="currentColor" opacity={0.12} stroke="none" />
      <circle cx="45" cy="80" r="3" fill="currentColor" opacity={0.08} stroke="none" />
    </svg>
  );
}

export function CommunityArt() {
  return (
    <svg {...SVG_DEFAULTS}>
      <circle cx="100" cy="60" r="35" strokeWidth={1.2} opacity={0.35} />
      <ellipse cx="100" cy="60" rx="14" ry="35" opacity={0.2} />
      <line x1="65" y1="60" x2="135" y2="60" opacity={0.2} />
      <ellipse cx="100" cy="45" rx="30" ry="8" opacity={0.15} />
      <ellipse cx="100" cy="75" rx="30" ry="8" opacity={0.15} />
      <circle cx="155" cy="30" r="6" opacity={0.2} />
      <circle cx="45" cy="40" r="5" opacity={0.15} />
      <circle cx="160" cy="90" r="4" opacity={0.12} />
      <line x1="135" y1="50" x2="150" y2="34" opacity={0.1} strokeDasharray="3 3" />
      <line x1="65" y1="55" x2="50" y2="42" opacity={0.08} strokeDasharray="3 3" />
      <line x1="130" y1="80" x2="156" y2="88" opacity={0.08} strokeDasharray="3 3" />
    </svg>
  );
}

export function CompatibilityArt() {
  return (
    <svg {...SVG_DEFAULTS}>
      <path d="M100 15 L140 30 L140 65 Q140 95 100 108 Q60 95 60 65 L60 30Z" strokeWidth={1.2} opacity={0.35} />
      <path d="M100 28 L130 38 L130 62 Q130 85 100 95 Q70 85 70 62 L70 38Z" opacity={0.12} />
      <path d="M82 58 L95 72 L120 46" strokeWidth={2} opacity={0.45} />
      <circle cx="155" cy="25" r="2.5" fill="currentColor" opacity={0.12} stroke="none" />
      <circle cx="45" cy="90" r="3" fill="currentColor" opacity={0.08} stroke="none" />
      <circle cx="160" cy="100" r="2" fill="currentColor" opacity={0.1} stroke="none" />
    </svg>
  );
}

export function SettingsArt() {
  return (
    <svg {...SVG_DEFAULTS}>
      <circle cx="100" cy="60" r="18" strokeWidth={1.2} opacity={0.35} />
      <circle cx="100" cy="60" r="8" opacity={0.25} />
      <line x1="100" y1="35" x2="100" y2="25" strokeWidth={3} opacity={0.25} strokeLinecap="round" />
      <line x1="100" y1="85" x2="100" y2="95" strokeWidth={3} opacity={0.25} strokeLinecap="round" />
      <line x1="75" y1="60" x2="65" y2="60" strokeWidth={3} opacity={0.25} strokeLinecap="round" />
      <line x1="125" y1="60" x2="135" y2="60" strokeWidth={3} opacity={0.25} strokeLinecap="round" />
      <line x1="82" y1="42" x2="75" y2="35" strokeWidth={3} opacity={0.2} strokeLinecap="round" />
      <line x1="118" y1="78" x2="125" y2="85" strokeWidth={3} opacity={0.2} strokeLinecap="round" />
      <line x1="118" y1="42" x2="125" y2="35" strokeWidth={3} opacity={0.2} strokeLinecap="round" />
      <line x1="82" y1="78" x2="75" y2="85" strokeWidth={3} opacity={0.2} strokeLinecap="round" />
      <circle cx="145" cy="35" r="10" opacity={0.15} />
      <circle cx="145" cy="35" r="4" opacity={0.12} />
      <rect x="45" y="40" width="20" height="8" rx="4" opacity={0.1} />
      <rect x="45" y="55" width="20" height="8" rx="4" opacity={0.08} />
      <rect x="45" y="70" width="20" height="8" rx="4" opacity={0.06} />
    </svg>
  );
}

export function HealthDashboardArt() {
  return (
    <svg {...SVG_DEFAULTS}>
      <rect x="45" y="20" width="110" height="80" rx="10" strokeWidth={1.2} opacity={0.3} />
      <path d="M55 60h18l8-20 12 40 8-20 10 0h34" strokeWidth={1.5} opacity={0.5} />
      <rect x="55" y="30" width="22" height="18" rx="4" opacity={0.2} />
      <rect x="82" y="30" width="22" height="18" rx="4" opacity={0.15} />
      <rect x="109" y="30" width="22" height="18" rx="4" opacity={0.12} />
      <rect x="136" y="30" width="14" height="18" rx="4" opacity={0.1} />
      <circle cx="63" cy="39" r="3" fill="currentColor" opacity={0.18} stroke="none" />
      <circle cx="90" cy="39" r="3" fill="currentColor" opacity={0.14} stroke="none" />
      <circle cx="117" cy="39" r="3" fill="currentColor" opacity={0.1} stroke="none" />
      <circle cx="165" cy="20" r="2.5" fill="currentColor" opacity={0.1} stroke="none" />
      <circle cx="38" cy="90" r="3" fill="currentColor" opacity={0.08} stroke="none" />
      <circle cx="170" cy="95" r="2" fill="currentColor" opacity={0.1} stroke="none" />
    </svg>
  );
}

export function LibraryArt() {
  return (
    <svg {...SVG_DEFAULTS}>
      {/* Shelf rows */}
      <line x1="35" y1="35" x2="165" y2="35" opacity={0.15} />
      <line x1="35" y1="65" x2="165" y2="65" opacity={0.12} />
      <line x1="35" y1="95" x2="165" y2="95" opacity={0.1} />
      {/* Book spines — top shelf */}
      <rect x="45" y="12" width="14" height="23" rx="2" strokeWidth={1.2} opacity={0.4} />
      <rect x="62" y="16" width="12" height="19" rx="2" opacity={0.3} />
      <rect x="77" y="10" width="16" height="25" rx="2" opacity={0.35} />
      <rect x="96" y="14" width="11" height="21" rx="2" opacity={0.25} />
      <rect x="110" y="11" width="15" height="24" rx="2" strokeWidth={1.2} opacity={0.38} />
      <rect x="128" y="15" width="13" height="20" rx="2" opacity={0.28} />
      {/* Book spines — middle shelf */}
      <rect x="50" y="42" width="16" height="23" rx="2" opacity={0.3} />
      <rect x="69" y="45" width="12" height="20" rx="2" opacity={0.22} />
      <rect x="84" y="40" width="14" height="25" rx="2" strokeWidth={1.2} opacity={0.35} />
      <rect x="101" y="44" width="18" height="21" rx="2" opacity={0.25} />
      <rect x="122" y="41" width="13" height="24" rx="2" opacity={0.3} />
      {/* Game controller accent */}
      <circle cx="155" cy="52" r="8" opacity={0.18} />
      <path d="M151 52h8M155 48v8" strokeWidth={1.5} opacity={0.2} />
      {/* Floating accent dots */}
      <circle cx="170" cy="20" r="2.5" fill="currentColor" opacity={0.12} stroke="none" />
      <circle cx="30" cy="80" r="3" fill="currentColor" opacity={0.08} stroke="none" />
      <circle cx="175" cy="100" r="2" fill="currentColor" opacity={0.1} stroke="none" />
    </svg>
  );
}
