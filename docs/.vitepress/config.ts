import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, type DefaultTheme, type PageData } from "vitepress";
import llmstxt from "vitepress-plugin-llms";

const __dirname = dirname(fileURLToPath(import.meta.url));

const SITE = "https://docker-exporter.tech";
const DESCRIPTION =
  "A tiny Rust Prometheus exporter for Docker container metrics. Correct memory working set on ARM64 & cgroup v2 (Raspberry Pi 5), ~7 MiB RAM, ~9 MB image, read-only socket, non-root.";
const OG_IMAGE = `${SITE}/og-share.png`;
const REPO = "https://github.com/dlepaux/docker-exporter";

// Keep the SoftwareApplication JSON-LD version in sync with the crate, automatically.
const version =
  /version\s*=\s*"([^"]+)"/.exec(
    readFileSync(resolve(__dirname, "..", "..", "Cargo.toml"), "utf8"),
  )?.[1] ?? "1.4.0";

// ── SEO/GEO helpers ─────────────────────────────────────────────────────────

/** Canonical URL matching VitePress's own sitemap scheme: directory-index pages
 *  keep a trailing slash, content pages drop the .md (cleanUrls). Keeping these
 *  in lockstep avoids a canonical-vs-sitemap mismatch on any section index page. */
function pageUrl(relativePath: string): string {
  if (relativePath === "index.md") return `${SITE}/`;
  const path = relativePath.replace(/\/index\.md$/, "/").replace(/\.md$/, "");
  return `${SITE}/${path}`;
}

function titleize(slug: string): string {
  return slug.replace(/-/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}

// There are no directory-index pages, so an intermediate breadcrumb crumb links
// to the section's first real page rather than a non-existent /section URL (404).
const SECTION_INDEX: Record<string, string> = {
  guide: "/guide/introduction",
  why: "/why/cadvisor-arm64-zero-memory",
  compare: "/compare/cadvisor",
};

function breadcrumb(pageData: PageData, url: string): object {
  const segs = pageData.relativePath
    .replace(/\.md$/, "")
    .split("/")
    .filter((s) => s && s !== "index");
  const items: object[] = [{ "@type": "ListItem", position: 1, name: "Home", item: `${SITE}/` }];
  segs.forEach((seg, i) => {
    const isLast = i === segs.length - 1;
    // Every ListItem needs a resolvable URL: the page's own for the leaf, the
    // mapped section landing for intermediates — never a dead /section link.
    const item = isLast ? url : SECTION_INDEX[seg] ? `${SITE}${SECTION_INDEX[seg]}` : undefined;
    items.push({
      "@type": "ListItem",
      position: i + 2,
      name: isLast ? pageData.title || titleize(seg) : titleize(seg),
      ...(item ? { item } : {}),
    });
  });
  return { "@context": "https://schema.org", "@type": "BreadcrumbList", itemListElement: items };
}

function softwareApp(): object {
  return {
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    name: "docker-exporter",
    description: DESCRIPTION,
    applicationCategory: "DeveloperApplication",
    operatingSystem: "Linux (amd64, arm64)",
    softwareVersion: version,
    license: "https://opensource.org/licenses/MIT",
    downloadUrl: `${REPO}/releases`,
    codeRepository: REPO,
    programmingLanguage: "Rust",
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
    author: { "@type": "Person", name: "David Lepaux", url: "https://github.com/dlepaux" },
  };
}

function article(pageData: PageData, url: string, desc: string): object {
  const modified = pageData.lastUpdated ? new Date(pageData.lastUpdated).toISOString() : undefined;
  return {
    "@context": "https://schema.org",
    // Co-typed: Article keeps Google rich-result eligibility; TechArticle carries
    // proficiency/dependency semantics for AI answer engines.
    "@type": ["Article", "TechArticle"],
    headline: pageData.title,
    description: desc,
    url,
    mainEntityOfPage: url,
    // dateModified tracks the git commit; datePublished only when an author sets an
    // explicit frontmatter `date` — never derived from lastUpdated, which would push
    // the "first published" date forward on every future edit.
    ...(modified ? { dateModified: modified } : {}),
    ...(pageData.frontmatter.date
      ? { datePublished: new Date(pageData.frontmatter.date as string).toISOString() }
      : {}),
    author: { "@type": "Person", name: "David Lepaux", url: "https://github.com/dlepaux" },
    publisher: {
      "@type": "Organization",
      name: "docker-exporter",
      logo: { "@type": "ImageObject", url: `${SITE}/favicon-96x96.png` },
    },
    image: OG_IMAGE,
  };
}

// ── Config ──────────────────────────────────────────────────────────────────

export default defineConfig({
  title: "docker-exporter",
  description: DESCRIPTION,
  lang: "en-US",

  // Apex custom domain → base stays "/". cleanUrls works natively on GitHub Pages.
  cleanUrls: true,
  lastUpdated: true,
  metaChunk: true,

  // The design spec lives under docs/ but must never become a published page.
  srcExclude: ["superpowers/**"],

  sitemap: { hostname: SITE },

  head: [
    ["meta", { name: "theme-color", content: "#14b8a6" }],
    ["meta", { name: "author", content: "David Lepaux" }],
    [
      "meta",
      {
        name: "keywords",
        content:
          "docker exporter, prometheus docker metrics, arm64, raspberry pi 5, cgroup v2, cadvisor alternative, lightweight container monitoring, rust",
      },
    ],
    ["meta", { property: "og:type", content: "website" }],
    ["meta", { property: "og:locale", content: "en_US" }],
    ["meta", { property: "og:site_name", content: "docker-exporter" }],
    ["meta", { property: "og:image", content: OG_IMAGE }],
    ["meta", { property: "og:image:width", content: "1200" }],
    ["meta", { property: "og:image:height", content: "630" }],
    ["meta", { name: "twitter:card", content: "summary_large_image" }],
    ["meta", { name: "twitter:image", content: OG_IMAGE }],
    ["link", { rel: "icon", href: "/favicon.ico", sizes: "any" }],
    ["link", { rel: "icon", type: "image/svg+xml", href: "/favicon.svg" }],
    ["link", { rel: "icon", type: "image/png", sizes: "96x96", href: "/favicon-96x96.png" }],
    ["link", { rel: "apple-touch-icon", sizes: "180x180", href: "/apple-touch-icon.png" }],
    ["link", { rel: "manifest", href: "/site.webmanifest" }],
  ],

  // Per-page SEO/GEO: canonical (no .html), OG title/description, JSON-LD.
  transformPageData(pageData) {
    const url = pageUrl(pageData.relativePath);
    const isHome = pageData.relativePath === "index.md";
    const desc = pageData.description || pageData.frontmatter.description || DESCRIPTION;
    const ogTitle = isHome
      ? "docker-exporter — Prometheus metrics for Docker on ARM64 & cgroup v2"
      : `${pageData.title} | docker-exporter`;

    const head = (pageData.frontmatter.head ??= []);
    head.push(
      ["link", { rel: "canonical", href: url }],
      ["meta", { property: "og:url", content: url }],
      ["meta", { property: "og:title", content: ogTitle }],
      ["meta", { property: "og:description", content: desc }],
      ["meta", { name: "twitter:title", content: ogTitle }],
      ["meta", { name: "twitter:description", content: desc }],
      ["script", { type: "application/ld+json" }, JSON.stringify(breadcrumb(pageData, url))],
      [
        "script",
        { type: "application/ld+json" },
        JSON.stringify(isHome ? softwareApp() : article(pageData, url, desc)),
      ],
    );
  },

  // GEO housekeeping: llms.txt / llms-full.txt for coding agents (not an SEO lever).
  vite: {
    plugins: [llmstxt({ domain: SITE })],
  },

  themeConfig: {
    logo: "/logo.svg",

    nav: [
      { text: "Guide", link: "/guide/introduction", activeMatch: "/guide/" },
      { text: "Metrics", link: "/guide/metrics" },
      { text: "vs cAdvisor", link: "/compare/cadvisor", activeMatch: "/compare/" },
      { text: "Why", link: "/why/cadvisor-arm64-zero-memory", activeMatch: "/why/" },
      {
        text: `v${version}`,
        items: [
          { text: "Changelog", link: `${REPO}/blob/main/changelog.md` },
          { text: "Releases", link: `${REPO}/releases` },
          { text: "Contributing", link: `${REPO}/blob/main/contributing.md` },
          { text: "Security policy", link: `${REPO}/blob/main/SECURITY.md` },
        ],
      },
    ],

    sidebar: {
      "/guide/": [
        {
          text: "Getting started",
          items: [
            { text: "Introduction", link: "/guide/introduction" },
            { text: "Installation", link: "/guide/installation" },
            { text: "Configuration", link: "/guide/configuration" },
          ],
        },
        {
          text: "Reference",
          items: [
            { text: "Metrics", link: "/guide/metrics" },
            { text: "Prometheus & Grafana", link: "/guide/prometheus-grafana" },
            { text: "Architecture", link: "/guide/architecture" },
          ],
        },
        {
          text: "Operations",
          items: [
            { text: "Troubleshooting", link: "/guide/troubleshooting" },
            { text: "Security", link: "/guide/security" },
          ],
        },
      ],
      "/why/": [
        {
          text: "Background",
          items: [
            { text: "The cAdvisor ARM64 memory bug", link: "/why/cadvisor-arm64-zero-memory" },
            { text: "Footprint benchmark", link: "/why/benchmark" },
          ],
        },
        { text: "Compare", items: [{ text: "vs cAdvisor", link: "/compare/cadvisor" }] },
      ],
      "/compare/": [
        {
          text: "Comparisons",
          items: [{ text: "docker-exporter vs cAdvisor", link: "/compare/cadvisor" }],
        },
        {
          text: "Background",
          items: [
            { text: "The cAdvisor ARM64 memory bug", link: "/why/cadvisor-arm64-zero-memory" },
            { text: "Footprint benchmark", link: "/why/benchmark" },
          ],
        },
      ],
    } satisfies DefaultTheme.Sidebar,

    socialLinks: [{ icon: "github", link: REPO }],

    editLink: {
      pattern: `${REPO}/edit/main/docs/:path`,
      text: "Edit this page on GitHub",
    },

    search: { provider: "local" },

    outline: "deep",

    footer: {
      message: `Released under the <a href="${REPO}/blob/main/license.md">MIT License</a>.`,
      copyright: "© 2026 David Lepaux",
    },
  },
});
