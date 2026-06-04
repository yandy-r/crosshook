import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from '@/test/setup';
import type { BreadcrumbSegment } from '../Breadcrumb';
import { Breadcrumb } from '../Breadcrumb';

const librarySegment: BreadcrumbSegment = { label: 'Library', onNavigate: vi.fn() };
const gameSegment: BreadcrumbSegment = { label: 'Test Game Alpha', onNavigate: vi.fn() };
const terminalSegment: BreadcrumbSegment = { label: 'Edit profile' };

describe('Breadcrumb', () => {
  it('renders segments in order inside nav > ol > li', () => {
    render(<Breadcrumb segments={[librarySegment, gameSegment, terminalSegment]} />);

    const nav = screen.getByRole('navigation', { name: 'Breadcrumb' });
    expect(nav).toBeInTheDocument();

    const list = nav.querySelector('ol');
    expect(list).toBeInTheDocument();

    const items = list?.querySelectorAll('li');
    expect(items).toHaveLength(3);
    expect(items?.[0]).toHaveTextContent('Library');
    expect(items?.[1]).toHaveTextContent('Test Game Alpha');
    expect(items?.[2]).toHaveTextContent('Edit profile');
  });

  it('renders terminal segment with aria-current="page" and not as a button', () => {
    render(<Breadcrumb segments={[librarySegment, terminalSegment]} />);

    const currentEl = screen.getByText('Edit profile');
    expect(currentEl).toHaveAttribute('aria-current', 'page');
    expect(currentEl.tagName).not.toBe('BUTTON');
  });

  it('renders exactly segments.length - 1 separators, all aria-hidden', () => {
    const segments = [librarySegment, gameSegment, terminalSegment];
    render(<Breadcrumb segments={segments} />);

    const separators = document.querySelectorAll('.crosshook-breadcrumb__separator');
    expect(separators).toHaveLength(segments.length - 1);
    for (const sep of separators) {
      expect(sep).toHaveAttribute('aria-hidden', 'true');
    }
  });

  it('clicking a crumb fires its onNavigate exactly once', async () => {
    const onNavigate = vi.fn();
    render(<Breadcrumb segments={[{ label: 'Library', onNavigate }, terminalSegment]} />);

    const crumb = screen.getByRole('button', { name: 'Library' });
    await userEvent.click(crumb);
    expect(onNavigate).toHaveBeenCalledTimes(1);
  });

  it('passes axe accessibility check', async () => {
    const { container } = render(<Breadcrumb segments={[librarySegment, gameSegment, terminalSegment]} />);

    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});
