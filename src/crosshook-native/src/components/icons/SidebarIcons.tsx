import type { SVGProps } from 'react';

type IconProps = SVGProps<SVGSVGElement>;

const defaults: IconProps = {
  width: 20,
  height: 20,
  viewBox: '0 0 20 20',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.5,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
};

export function ProfilesIcon(props: IconProps) {
  return (
    <svg {...defaults} {...props}>
      <circle cx="10" cy="6.5" r="3.5" />
      <path d="M3.5 17.5v-1a5.5 5.5 0 0 1 13 0v1" />
    </svg>
  );
}

export function LaunchIcon(props: IconProps) {
  return (
    <svg {...defaults} {...props}>
      <path d="M6 3.5v13l10-6.5z" />
    </svg>
  );
}

export function InstallIcon(props: IconProps) {
  return (
    <svg {...defaults} {...props}>
      <path d="M10 2.5v10m0 0-3.5-3.5M10 12.5l3.5-3.5" />
      <path d="M3.5 14.5v2a1 1 0 0 0 1 1h11a1 1 0 0 0 1-1v-2" />
    </svg>
  );
}

export function BrowseIcon(props: IconProps) {
  return (
    <svg {...defaults} {...props}>
      <circle cx="10" cy="10" r="7.5" />
      <ellipse cx="10" cy="10" rx="3" ry="7.5" />
      <path d="M2.5 10h15" />
    </svg>
  );
}

export function CompatibilityIcon(props: IconProps) {
  return (
    <svg {...defaults} {...props}>
      <path d="M10 1.5 2.5 5v5.5c0 4 3.2 7.2 7.5 8 4.3-.8 7.5-4 7.5-8V5z" />
      <path d="m7 10 2 2 4-4" />
    </svg>
  );
}

export function SettingsIcon(props: IconProps) {
  return (
    <svg {...defaults} {...props}>
      <circle cx="10" cy="10" r="2.5" />
      <path d="M10 1.5v2m0 13v2m-6-8.5h2m13 0h-2M4 4l1.5 1.5M14.5 14.5 16 16M4 16l1.5-1.5M14.5 5.5 16 4" />
    </svg>
  );
}
