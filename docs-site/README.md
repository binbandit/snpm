# docs-site

This is a Next.js application generated with
[Create Fumadocs](https://github.com/fuma-nama/fumadocs).

Run development server:

```bash
npm run dev
# or
pnpm dev
# or
yarn dev
```

Open http://localhost:3000 with your browser to see the result.

## Explore

In the project, you can see:

- `lib/source.ts`: Code for content source adapter, [`loader()`](https://fumadocs.dev/docs/headless/source-api) provides the interface to access your content.
- `lib/layout.shared.tsx`: Shared options for layouts, optional but preferred to keep.

| Route                     | Description                                            |
| ------------------------- | ------------------------------------------------------ |
| `app/(home)`              | The route group for your landing page and other pages. |
| `app/docs`                | The documentation layout and pages.                    |
| `app/api/search/route.ts` | The Route Handler for search.                          |

### Fumadocs MDX

A `source.config.ts` config file has been included, you can customise different options like frontmatter schema.

Read the [Introduction](https://fumadocs.dev/docs/mdx) for further details.

## Deployment

The docs site is statically exported and published by `.github/workflows/deploy-docs.yml` to GitHub Pages on pushes to `main`.

Production is served from `https://snpm.io`, so the workflow builds without a `NEXT_PUBLIC_BASE_PATH`. `public/.nojekyll` keeps GitHub Pages from hiding Next.js `_next/` assets. `public/CNAME` is included for static-host compatibility, while the GitHub Pages custom-domain setting remains the source of truth for Actions deployments.

One-time Pages and DNS settings:

- GitHub Pages source: GitHub Actions.
- GitHub Pages custom domain: `snpm.io`.
- Apex `A` records: `185.199.108.153`, `185.199.109.153`, `185.199.110.153`, `185.199.111.153`.
- `www` CNAME: `binbandit.github.io`.
- Enable HTTPS enforcement after GitHub provisions the certificate.

## Learn More

To learn more about Next.js and Fumadocs, take a look at the following
resources:

- [Next.js Documentation](https://nextjs.org/docs) - learn about Next.js
  features and API.
- [Learn Next.js](https://nextjs.org/learn) - an interactive Next.js tutorial.
- [Fumadocs](https://fumadocs.dev) - learn about Fumadocs
