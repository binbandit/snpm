import { NextResponse } from 'next/server';

export const dynamic = 'force-dynamic';
export const revalidate = 64800; // Cache for 18 hours

export async function GET() {
  try {
    const response = await fetch(
      'https://api.github.com/repos/binbandit/snpm/releases/latest',
      {
        headers: {
          'Accept': 'application/vnd.github.v3+json',
          'User-Agent': 'snpm-docs-site',
        },
        next: { revalidate: 64800 }, // Cache for 18 hours
      }
    );

    if (!response.ok) {
      throw new Error('Failed to fetch version');
    }

    const data = await response.json();
    const version = data.tag_name?.replace(/^v/, '') || '2025.12.3';

    return NextResponse.json({ version });
  } catch (error) {
    console.error('Error fetching version:', error);
    // Fallback to a default version
    return NextResponse.json({ version: '2025.12.3' });
  }
}
