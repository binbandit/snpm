import { source } from '@/lib/source';
import { DocsLayout } from 'fumadocs-ui/layouts/docs';
import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';
import { Package } from 'lucide-react';

const docsOptions: BaseLayoutProps = {
  nav: {
    title: (
      <div className="flex items-center gap-2.5">
        <div className="bg-gradient-to-br from-teal-600 to-teal-700 p-2.5 rounded-lg shadow-sm">
          <Package className="h-5 w-5 text-white" />
        </div>
        <span className="text-lg text-gray-900 dark:text-[#f5f1e8]">snpm</span>
        <span className="text-xs text-gray-600 dark:text-[#c9b89a] bg-[#e8dcc8] dark:bg-[#3a2d1d] px-2.5 py-1 rounded-full">
          v2025.12.3
        </span>
      </div>
    ),
  },
  themeSwitch: { enabled: true },
  githubUrl: 'https://github.com/binbandit/snpm',
  searchToggle: { enabled: false },
};

export default function Layout({ children }: LayoutProps<'/docs'>) {
  return (
    <DocsLayout tree={source.pageTree} {...docsOptions}>
      {children}
    </DocsLayout>
  );
}
