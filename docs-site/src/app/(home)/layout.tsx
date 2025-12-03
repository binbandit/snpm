import { HomeLayout } from '@/components/layout/home';
import { baseOptions } from '@/lib/layout.shared';
import type { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'snpm - The Speedy Way to Manage Packages',
  description: 'A drop-in replacement for npm, yarn, and pnpm. Faster installs, simpler codebase, deterministic builds—everything you need, nothing you don\'t.',
  openGraph: {
    title: 'snpm - The Speedy Way to Manage Packages',
    description: 'A drop-in replacement for npm, yarn, and pnpm. Built with Rust for unmatched speed.',
    type: 'website',
  },
};

export default function Layout({ children }: { children: React.ReactNode }) {
  return <HomeLayout {...baseOptions()}>{children}</HomeLayout>;
}
