"use client";

import { usePrefersDark, useStoredTheme } from "../lib/client-hooks";

/**
 * Explicit theme choice, stamped on <html data-theme>. With no choice stored
 * the attribute stays absent so the page follows prefers-color-scheme — the
 * CSS defines all three states, so this only has to persist the override.
 */
export default function ThemeToggle() {
  const stored = useStoredTheme();
  const systemDark = usePrefersDark();
  const isDark = stored === "dark" || (stored === null && systemDark);

  const toggle = () => {
    const next = isDark ? "light" : "dark";
    document.documentElement.dataset.theme = next;
    try {
      localStorage.setItem("theme", next);
    } catch {
      /* private mode — the attribute still applies for this session */
    }
    // `storage` only fires in other tabs, so nudge this one's subscribers.
    window.dispatchEvent(new Event("themechange"));
  };

  return (
    <button
      className="icon-btn"
      onClick={toggle}
      aria-label="Toggle colour theme"
      title="Toggle colour theme"
    >
      {/* Both glyphs ship; CSS shows the one matching the resolved theme so
          the button is correct before hydration. */}
      <svg
        width="17"
        height="17"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.9"
        strokeLinecap="round"
        aria-hidden="true"
      >
        <circle cx="12" cy="12" r="4.2" />
        <path d="M12 2.6v2.2M12 19.2v2.2M21.4 12h-2.2M4.8 12H2.6M18.6 5.4l-1.6 1.6M7 17l-1.6 1.6M18.6 18.6L17 17M7 7L5.4 5.4" />
      </svg>
    </button>
  );
}
