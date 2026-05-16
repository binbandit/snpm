import { RootProvider } from "fumadocs-ui/provider/next";
import "./global.css";
import type { Metadata } from "next";
import { Inter } from "next/font/google";

const inter = Inter({
  subsets: ["latin"],
});

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? "https://snpm.io";

export const metadata: Metadata = {
  metadataBase: new URL(siteUrl),
  applicationName: "snpm",
  alternates: {
    canonical: "/",
  },
  openGraph: {
    siteName: "snpm",
    url: "/",
  },
  twitter: {
    card: "summary_large_image",
  },
};

export default function Layout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" className={inter.className} suppressHydrationWarning>
      <body className="flex min-h-screen flex-col bg-fd-background text-fd-foreground antialiased">
        <RootProvider
          search={{
            options: {
              type: "static",
            },
          }}
        >
          {children}
        </RootProvider>
      </body>
    </html>
  );
}
