import { getLLMText } from '@/lib/get-llm-text';
import { source } from '@/lib/source';
import { notFound } from 'next/navigation';

export const revalidate = false;

export async function GET(_req: Request, { params }: RouteContext<'/llms.mdx/docs/[...slug]'>) {
  const { slug } = await params;
  const pageSlug = slug.length === 1 && slug[0] === 'index' ? [] : slug;
  const page = source.getPage(pageSlug);
  if (!page) notFound();

  return new Response(await getLLMText(page), {
    headers: {
      'Content-Type': 'text/markdown',
    },
  });
}

export function generateStaticParams() {
  return source.generateParams().map((params) => {
    if (Array.isArray(params.slug) && params.slug.length === 0) {
      return { slug: ['index'] };
    }

    return params;
  });
}
