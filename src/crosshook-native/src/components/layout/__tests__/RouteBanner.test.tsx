import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { BreadcrumbSegment } from '../Breadcrumb';
import { RouteBanner } from '../RouteBanner';
import { ROUTE_METADATA } from '../routeMetadata';

describe('RouteBanner', () => {
  describe('without trail', () => {
    it('renders the static eyebrow text from ROUTE_METADATA', () => {
      render(<RouteBanner route="install" />);

      const expectedEyebrow = ROUTE_METADATA.install.sectionEyebrow;
      expect(screen.getByText(expectedEyebrow)).toBeInTheDocument();
    });

    it('does not render a breadcrumb navigation element', () => {
      render(<RouteBanner route="install" />);

      expect(screen.queryByRole('navigation', { name: 'Breadcrumb' })).toBeNull();
    });

    it('renders the title heading', () => {
      render(<RouteBanner route="install" />);

      expect(screen.getByRole('heading', { name: ROUTE_METADATA.install.bannerTitle })).toBeInTheDocument();
    });
  });

  describe('with trail', () => {
    const trail: BreadcrumbSegment[] = [{ label: 'Library', onNavigate: vi.fn() }, { label: 'Test Game' }];

    it('renders the breadcrumb navigation element', () => {
      render(<RouteBanner route="install" trail={trail} />);

      expect(screen.getByRole('navigation', { name: 'Breadcrumb' })).toBeInTheDocument();
    });

    it('does not render the static eyebrow <p> element', () => {
      const { container } = render(<RouteBanner route="install" trail={trail} />);

      expect(container.querySelector('p.crosshook-route-banner__eyebrow')).toBeNull();
    });

    it('still renders the title heading', () => {
      render(<RouteBanner route="install" trail={trail} />);

      expect(screen.getByRole('heading', { name: ROUTE_METADATA.install.bannerTitle })).toBeInTheDocument();
    });

    it('still renders the summary text', () => {
      render(<RouteBanner route="install" trail={trail} />);

      expect(screen.getByText(ROUTE_METADATA.install.bannerSummary)).toBeInTheDocument();
    });
  });

  describe('with empty trail array', () => {
    it('falls back to the static eyebrow <p> when trail is an empty array', () => {
      render(<RouteBanner route="library" trail={[]} />);

      const expectedEyebrow = ROUTE_METADATA.library.sectionEyebrow;
      expect(screen.getByText(expectedEyebrow)).toBeInTheDocument();
      expect(screen.queryByRole('navigation', { name: 'Breadcrumb' })).toBeNull();
    });
  });
});
