// Brand design tokens — the single source of truth for docker-exporter's mark.
// Edit here, then `npm run build:brand` regenerates every asset (logo, favicon,
// apple-touch, OG image) from these values. No font dependency: the mark is pure
// geometry (a container tile + a metric beat), so it renders identically on
// macOS and Ubuntu CI.

// Signal-teal palette — deliberately not Docker-blue or Prometheus/Grafana-orange.
export const TEAL = "#14b8a6"; // primary brand
export const TEAL_DEEP = "#0d9488"; // gradient shadow
export const CYAN = "#22d3ee"; // gradient highlight (the "signal")
export const SLATE = "#0b1220"; // deep background (OG, apple-touch)
export const INK = "#e2e8f0"; // light foreground text

// The metric beat — one clean heartbeat blip: flat, up to a peak, down through a
// valley, back to flat. Symmetric and legible from 16px up.
export function beatPoints(cx: number, width: number, baseline: number, amp: number): string {
  const x0 = cx - width / 2;
  const x1 = cx + width / 2;
  const p: Array<[number, number]> = [
    [x0, baseline],
    [cx - width * 0.16, baseline],
    [cx - width * 0.02, baseline - amp], // peak
    [cx + width * 0.1, baseline + amp], // valley
    [cx + width * 0.22, baseline],
    [x1, baseline],
  ];
  return p.map(([x, y]) => `${round(x)},${round(y)}`).join(" ");
}

function round(n: number): number {
  return Math.round(n * 100) / 100;
}
