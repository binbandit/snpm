import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';
import { Package, ArrowRight } from 'lucide-react';

export function baseOptions(): BaseLayoutProps {
  return {
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
      transparentMode: 'top',
    },
    themeSwitch: { enabled: true },
    githubUrl: 'https://github.com/binbandit/snpm',
    searchToggle: { enabled: false },
    links: [
      {
        text: 'Features',
        url: '/#features',
        active: 'none',
      },
      {
        text: 'Performance',
        url: '/#performance',
        active: 'none',
      },
      {
        text: 'Comparison',
        url: '/#comparison',
        active: 'none',
      },
      {
        type: 'main',
        text: 'Docs',
        url: '/docs',
        active: 'none',
      },
      {
        type: 'button',
        text: 'Get Started',
        url: '/docs/installation',
        icon: <ArrowRight className="h-3.5 w-3.5" />,
        secondary: true,
      },
    ],
  };
}
