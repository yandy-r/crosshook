import type { ReactNode, SVGProps } from 'react';

interface PageBannerProps {
  eyebrow: string;
  title: string;
  copy: string;
  illustration: ReactNode;
}

export function PageBanner({ eyebrow, title, copy, illustration }: PageBannerProps) {
  return (
    <header className="crosshook-page-banner">
      <div className="crosshook-page-banner__text">
        <div className="crosshook-heading-eyebrow">{eyebrow}</div>
        <h1 className="crosshook-heading-title">{title}</h1>
        <p className="crosshook-heading-copy">{copy}</p>
      </div>
      <div className="crosshook-page-banner__art" aria-hidden="true">
        {illustration}
      </div>
    </header>
  );
}

/* ── Per-page decorative illustrations ──────────────────────────────── */

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
      {/* Main profile card */}
      <rect x="60" y="20" width="80" height="80" rx="12" strokeWidth={1.2} opacity={0.5} />
      <circle cx="100" cy="48" r="14" opacity={0.45} />
      <path d="M80 78a20 20 0 0 1 40 0" opacity={0.4} />
      {/* Stacked cards behind */}
      <rect x="48" y="28" width="80" height="80" rx="12" opacity={0.15} />
      <rect x="72" y="12" width="80" height="80" rx="12" opacity={0.1} />
      {/* Accent dots */}
      <circle cx="155" cy="30" r="3" fill="currentColor" opacity={0.12} stroke="none" />
      <circle cx="40" cy="90" r="4" fill="currentColor" opacity={0.1} stroke="none" />
      <circle cx="170" cy="85" r="2" fill="currentColor" opacity={0.15} stroke="none" />
    </svg>
  );
}

export function LaunchArt() {
  return (
    <svg {...SVG_DEFAULTS}>
      {/* Large play triangle */}
      <path d="M65 25 L150 60 L65 95Z" strokeWidth={1.2} opacity={0.4} />
      {/* Motion lines */}
      <line x1="40" y1="45" x2="55" y2="45" opacity={0.25} />
      <line x1="35" y1="60" x2="55" y2="60" opacity={0.3} />
      <line x1="40" y1="75" x2="55" y2="75" opacity={0.25} />
      {/* Exhaust/thrust trails */}
      <path d="M60 40 Q45 60 60 80" opacity={0.15} strokeWidth={1.5} />
      <path d="M50 35 Q30 60 50 85" opacity={0.08} strokeWidth={2} />
      {/* Accent particles */}
      <circle cx="160" cy="35" r="2.5" fill="currentColor" opacity={0.15} stroke="none" />
      <circle cx="165" cy="80" r="1.5" fill="currentColor" opacity={0.12} stroke="none" />
      <circle cx="30" cy="30" r="3" fill="currentColor" opacity={0.08} stroke="none" />
    </svg>
  );
}

export function InstallArt() {
  return (
    <svg {...SVG_DEFAULTS}>
      {/* Download box */}
      <rect x="65" y="45" width="70" height="50" rx="10" strokeWidth={1.2} opacity={0.4} />
      {/* Arrow shaft */}
      <line x1="100" y1="10" x2="100" y2="55" strokeWidth={1.5} opacity={0.45} />
      {/* Arrow head */}
      <path d="M88 43 L100 58 L112 43" strokeWidth={1.5} opacity={0.45} />
      {/* Open top of box */}
      <path d="M65 55 Q100 40 135 55" opacity={0.2} />
      {/* Progress bar */}
      <rect x="75" y="78" width="50" height="4" rx="2" opacity={0.2} />
      <rect x="75" y="78" width="30" height="4" rx="2" fill="currentColor" opacity={0.15} stroke="none" />
      {/* Dots */}
      <circle cx="155" cy="25" r="2" fill="currentColor" opacity={0.12} stroke="none" />
      <circle cx="45" cy="80" r="3" fill="currentColor" opacity={0.08} stroke="none" />
    </svg>
  );
}

export function CommunityArt() {
  return (
    <svg {...SVG_DEFAULTS}>
      {/* Central globe */}
      <circle cx="100" cy="60" r="35" strokeWidth={1.2} opacity={0.35} />
      <ellipse cx="100" cy="60" rx="14" ry="35" opacity={0.2} />
      <line x1="65" y1="60" x2="135" y2="60" opacity={0.2} />
      <ellipse cx="100" cy="45" rx="30" ry="8" opacity={0.15} />
      <ellipse cx="100" cy="75" rx="30" ry="8" opacity={0.15} />
      {/* Connection nodes */}
      <circle cx="155" cy="30" r="6" opacity={0.2} />
      <circle cx="45" cy="40" r="5" opacity={0.15} />
      <circle cx="160" cy="90" r="4" opacity={0.12} />
      {/* Connection lines to globe */}
      <line x1="135" y1="50" x2="150" y2="34" opacity={0.1} strokeDasharray="3 3" />
      <line x1="65" y1="55" x2="50" y2="42" opacity={0.08} strokeDasharray="3 3" />
      <line x1="130" y1="80" x2="156" y2="88" opacity={0.08} strokeDasharray="3 3" />
    </svg>
  );
}

export function CompatibilityArt() {
  return (
    <svg {...SVG_DEFAULTS}>
      {/* Shield */}
      <path d="M100 15 L140 30 L140 65 Q140 95 100 108 Q60 95 60 65 L60 30Z" strokeWidth={1.2} opacity={0.35} />
      {/* Inner shield line */}
      <path d="M100 28 L130 38 L130 62 Q130 85 100 95 Q70 85 70 62 L70 38Z" opacity={0.12} />
      {/* Checkmark */}
      <path d="M82 58 L95 72 L120 46" strokeWidth={2} opacity={0.45} />
      {/* Accent dots */}
      <circle cx="155" cy="25" r="2.5" fill="currentColor" opacity={0.12} stroke="none" />
      <circle cx="45" cy="90" r="3" fill="currentColor" opacity={0.08} stroke="none" />
      <circle cx="160" cy="100" r="2" fill="currentColor" opacity={0.1} stroke="none" />
    </svg>
  );
}

export function SettingsArt() {
  return (
    <svg {...SVG_DEFAULTS}>
      {/* Main gear */}
      <circle cx="100" cy="60" r="18" strokeWidth={1.2} opacity={0.35} />
      <circle cx="100" cy="60" r="8" opacity={0.25} />
      {/* Gear teeth as radiating lines */}
      <line x1="100" y1="35" x2="100" y2="25" strokeWidth={3} opacity={0.25} strokeLinecap="round" />
      <line x1="100" y1="85" x2="100" y2="95" strokeWidth={3} opacity={0.25} strokeLinecap="round" />
      <line x1="75" y1="60" x2="65" y2="60" strokeWidth={3} opacity={0.25} strokeLinecap="round" />
      <line x1="125" y1="60" x2="135" y2="60" strokeWidth={3} opacity={0.25} strokeLinecap="round" />
      <line x1="82" y1="42" x2="75" y2="35" strokeWidth={3} opacity={0.2} strokeLinecap="round" />
      <line x1="118" y1="78" x2="125" y2="85" strokeWidth={3} opacity={0.2} strokeLinecap="round" />
      <line x1="118" y1="42" x2="125" y2="35" strokeWidth={3} opacity={0.2} strokeLinecap="round" />
      <line x1="82" y1="78" x2="75" y2="85" strokeWidth={3} opacity={0.2} strokeLinecap="round" />
      {/* Small gear */}
      <circle cx="145" cy="35" r="10" opacity={0.15} />
      <circle cx="145" cy="35" r="4" opacity={0.12} />
      {/* Toggle sliders hint */}
      <rect x="45" y="40" width="20" height="8" rx="4" opacity={0.1} />
      <rect x="45" y="55" width="20" height="8" rx="4" opacity={0.08} />
      <rect x="45" y="70" width="20" height="8" rx="4" opacity={0.06} />
    </svg>
  );
}

export default PageBanner;
