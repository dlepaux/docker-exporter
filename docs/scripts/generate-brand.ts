#!/usr/bin/env tsx
// Generate every brand asset from the tokens in brand-paths.ts.
//   Run:    npm run build:brand   (also runs automatically before dev/build)
//   Writes: brand/icon.svg, brand/favicon.svg            (committed SVG source of record)
//           docs/public/{logo,favicon}.svg               (served by VitePress)
//           docs/public/{favicon-96x96,favicon.ico,apple-touch-icon,og-share}.png

import { mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { CYAN, INK, SLATE, TEAL, TEAL_DEEP, beatPoints } from "./brand-paths.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

const gradient = (id: string) => `
    <linearGradient id="${id}" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="${CYAN}"/>
      <stop offset="55%" stop-color="${TEAL}"/>
      <stop offset="100%" stop-color="${TEAL_DEEP}"/>
    </linearGradient>`;

// Logo — one container tile with a clean metric beat. Used in nav, hero, OG.
function buildLogo(): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128" fill="none">
  <defs>${gradient("g")}</defs>
  <rect x="14" y="14" width="100" height="100" rx="26" fill="url(#g)"/>
  <polyline points="${beatPoints(64, 74, 64, 19)}"
    fill="none" stroke="#ffffff" stroke-width="8"
    stroke-linecap="round" stroke-linejoin="round"/>
</svg>`;
}

// Favicon — same mark, slightly heavier stroke for legibility at 16px.
function buildFavicon(): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128" fill="none">
  <defs>${gradient("gf")}</defs>
  <rect x="10" y="10" width="108" height="108" rx="26" fill="url(#gf)"/>
  <polyline points="${beatPoints(64, 78, 64, 21)}"
    fill="none" stroke="#ffffff" stroke-width="10"
    stroke-linecap="round" stroke-linejoin="round"/>
</svg>`;
}

// Apple-touch — full-bleed (iOS applies its own rounding/masking).
function buildAppleTouch(): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 180 180" fill="none">
  <defs>${gradient("ga")}</defs>
  <rect x="0" y="0" width="180" height="180" fill="url(#ga)"/>
  <polyline points="${beatPoints(90, 108, 90, 28)}"
    fill="none" stroke="#ffffff" stroke-width="13"
    stroke-linecap="round" stroke-linejoin="round"/>
</svg>`;
}

// OG share image (1200x630) — mark + wordmark + tagline on a deep-slate field.
function buildOg(): string {
  const w = 1200;
  const h = 630;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">
  <defs>
    ${gradient("go")}
    <radialGradient id="glow" cx="50%" cy="34%" r="55%">
      <stop offset="0%" stop-color="${TEAL}" stop-opacity="0.22"/>
      <stop offset="100%" stop-color="${TEAL}" stop-opacity="0"/>
    </radialGradient>
    <pattern id="grid" width="34" height="34" patternUnits="userSpaceOnUse">
      <path d="M34 0H0V34" fill="none" stroke="#1e293b" stroke-width="1"/>
    </pattern>
  </defs>
  <rect width="${w}" height="${h}" fill="${SLATE}"/>
  <rect width="${w}" height="${h}" fill="url(#grid)" opacity="0.5"/>
  <rect width="${w}" height="${h}" fill="url(#glow)"/>

  <!-- mark -->
  <g transform="translate(120, 205) scale(1.7)">
    <rect x="14" y="14" width="100" height="100" rx="26" fill="url(#go)"/>
    <polyline points="${beatPoints(64, 74, 64, 19)}"
      fill="none" stroke="#ffffff" stroke-width="8"
      stroke-linecap="round" stroke-linejoin="round"/>
  </g>

  <!-- wordmark -->
  <text x="368" y="250"
    font-family="ui-monospace, 'SF Mono', Menlo, 'DejaVu Sans Mono', monospace"
    font-size="66" font-weight="700" fill="${INK}"
  >docker-exporter</text>

  <!-- tagline -->
  <text x="370" y="308"
    font-family="ui-monospace, 'SF Mono', Menlo, 'DejaVu Sans Mono', monospace"
    font-size="25" font-weight="400" fill="${TEAL}"
  >Prometheus metrics for Docker on ARM64 &amp; cgroup v2</text>

  <!-- stat strip -->
  <g font-family="ui-monospace, 'SF Mono', Menlo, 'DejaVu Sans Mono', monospace">
    <text x="370" y="408" font-size="26" fill="#94a3b8">~7 MiB RAM</text>
    <text x="568" y="408" font-size="26" fill="#94a3b8">~9 MB image</text>
    <text x="768" y="408" font-size="26" fill="#94a3b8">read-only socket</text>
  </g>

  <rect x="370" y="430" width="712" height="1" fill="#1e293b"/>
  <text x="370" y="470"
    font-family="ui-monospace, 'SF Mono', Menlo, 'DejaVu Sans Mono', monospace"
    font-size="24" fill="#64748b"
  >docker-exporter.tech</text>
</svg>`;
}

async function rasterize(svg: string, size: { w: number; h: number }, out: string): Promise<void> {
  const { default: sharp } = await import("sharp");
  await sharp(Buffer.from(svg))
    .resize(size.w, size.h)
    .png({ compressionLevel: 9, quality: 90 })
    .toFile(out);
  console.log(`wrote ${out}`);
}

// Wrap a PNG buffer in a single-image ICO container (ICONDIR + ICONDIRENTRY + PNG),
// so favicon.ico is a spec-valid icon file, not a PNG renamed .ico.
function pngToIco(png: Buffer, size: number): Buffer {
  const head = Buffer.alloc(22);
  head.writeUInt16LE(0, 0); // reserved
  head.writeUInt16LE(1, 2); // type: icon
  head.writeUInt16LE(1, 4); // one image
  head.writeUInt8(size >= 256 ? 0 : size, 6); // width (0 means 256)
  head.writeUInt8(size >= 256 ? 0 : size, 7); // height
  head.writeUInt8(0, 8); // palette size
  head.writeUInt8(0, 9); // reserved
  head.writeUInt16LE(1, 10); // color planes
  head.writeUInt16LE(32, 12); // bits per pixel
  head.writeUInt32LE(png.length, 14); // image byte size
  head.writeUInt32LE(22, 18); // offset to image data
  return Buffer.concat([head, png]);
}

const brandDir = resolve(__dirname, "..", "..", "brand");
const publicDir = resolve(__dirname, "..", "public");
mkdirSync(brandDir, { recursive: true });
mkdirSync(publicDir, { recursive: true });

const logo = buildLogo();
const favicon = buildFavicon();

writeFileSync(resolve(brandDir, "icon.svg"), logo);
writeFileSync(resolve(brandDir, "favicon.svg"), favicon);
writeFileSync(resolve(publicDir, "logo.svg"), logo);
writeFileSync(resolve(publicDir, "favicon.svg"), favicon);
console.log("wrote brand/{icon,favicon}.svg and public/{logo,favicon}.svg");

await rasterize(favicon, { w: 96, h: 96 }, resolve(publicDir, "favicon-96x96.png"));
// favicon.ico — a real 32×32 ICO container (stops the automatic /favicon.ico 404).
const { default: sharpIco } = await import("sharp");
const icoPng = await sharpIco(Buffer.from(favicon)).resize(32, 32).png().toBuffer();
writeFileSync(resolve(publicDir, "favicon.ico"), pngToIco(icoPng, 32));
console.log("wrote favicon.ico");
await rasterize(buildAppleTouch(), { w: 180, h: 180 }, resolve(publicDir, "apple-touch-icon.png"));
await rasterize(buildOg(), { w: 1200, h: 630 }, resolve(publicDir, "og-share.png"));
console.log("brand generation done");
