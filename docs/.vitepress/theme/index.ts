import DefaultTheme from "vitepress/theme";
import "./custom.css";

export default {
  extends: DefaultTheme,
  enhanceApp({ router }) {
    // Client-only: gtag's config sends the first page_view, but VitePress is an
    // SPA, so client-side navigations don't reload the page — report them to GA4.
    if (import.meta.env.SSR) return;
    router.onAfterRouteChanged = (to: string) => {
      const w = window as unknown as { gtag?: (...args: unknown[]) => void };
      if (typeof w.gtag === "function") {
        w.gtag("event", "page_view", { page_path: to, page_title: document.title });
      }
    };
  },
};
