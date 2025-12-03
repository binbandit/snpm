import { NextResponse } from 'next/server';
import { readFile } from 'fs/promises';
import { join } from 'path';

export const dynamic = 'force-dynamic';

export async function GET() {
  try {
    const filePath = join(process.cwd(), 'public', 'snpm-acronyms.txt');
    const content = await readFile(filePath, 'utf-8');
    const acronyms = content.split('\n').filter(line => line.trim());
    
    if (acronyms.length === 0) {
      throw new Error('No acronyms found');
    }

    const randomAcronym = acronyms[Math.floor(Math.random() * acronyms.length)];
    
    return NextResponse.json({ acronym: randomAcronym });
  } catch (error) {
    console.error('Error reading acronyms:', error);
    return NextResponse.json({ acronym: 'Suddenly Not Panicking Manager' });
  }
}
