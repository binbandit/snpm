import { HomeLayout } from '@/components/layout/home';
import { baseOptions } from '@/lib/layout.shared';
import type { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'snpm - A Rust-native package manager for JavaScript',
  description: 'snpm is a Rust-native package manager shaped around pnpm-style workflows: shared package store, workspace fan-out, security-first defaults, and lockfile imports from pnpm, Bun, Yarn, and npm.',
  openGraph: {
    title: 'snpm - A Rust-native package manager for JavaScript',
    description: 'Shared package store, workspace fan-out, security-first defaults, and lockfile imports from pnpm, Bun, Yarn, and npm.',
    type: 'website',
  },
};

export default function Layout({ children }: { children: React.ReactNode }) {
  return <HomeLayout {...baseOptions()}>{children}</HomeLayout>;
}
