import { getPageImage, source } from '@/lib/source';
import Link from 'next/link';
import {
  DocsBody,
  DocsDescription,
  DocsPage,
  DocsTitle,
} from 'fumadocs-ui/layouts/docs/page';
import { notFound } from 'next/navigation';
import { getMDXComponents } from '@/mdx-components';
import type { Metadata } from 'next';
import { createRelativeLink } from 'fumadocs-ui/mdx';

export default async function Page(props: PageProps<'/docs/[[...slug]]'>) {
  const params = await props.params;
  const page = source.getPage(params.slug);
  if (!page) notFound();

  const MDX = page.data.body;
  const markdownPath =
    page.slugs.length === 0
      ? '/llms.mdx/docs/index'
      : `/llms.mdx/docs/${page.slugs.join('/')}`;
  const githubPath =
    page.slugs.length === 0
      ? 'index.mdx'
      : `${page.slugs.join('/')}.mdx`;
  const githubUrl =
    `https://github.com/binbandit/snpm/blob/main/docs-site/content/docs/${githubPath}`;

  return (
    <DocsPage toc={page.data.toc} full={page.data.full}>
      <DocsTitle>{page.data.title}</DocsTitle>
      <DocsDescription>{page.data.description}</DocsDescription>
      <div className="mb-6 flex flex-wrap items-center gap-3 border-b border-fd-border pb-6 pt-1 text-sm">
        <Link
          href={markdownPath}
          className="rounded-md border border-fd-border px-3 py-1.5 transition-colors hover:bg-fd-accent"
        >
          View as Markdown
        </Link>
        <Link
          href={githubUrl}
          target="_blank"
          rel="noreferrer"
          className="rounded-md border border-fd-border px-3 py-1.5 transition-colors hover:bg-fd-accent"
        >
          View source
        </Link>
      </div>
      <DocsBody>
        <MDX
          components={getMDXComponents({
            // this allows you to link to other pages with relative file paths
            a: createRelativeLink(source, page),
          })}
        />
      </DocsBody>
    </DocsPage>
  );
}

export async function generateStaticParams() {
  return source.generateParams();
}

export async function generateMetadata(
  props: PageProps<'/docs/[[...slug]]'>,
): Promise<Metadata> {
  const params = await props.params;
  const page = source.getPage(params.slug);
  if (!page) notFound();

  return {
    title: page.data.title,
    description: page.data.description,
    openGraph: {
      images: getPageImage(page).url,
    },
  };
}
