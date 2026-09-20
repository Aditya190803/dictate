"use client";

import { useEffect, useRef, type CSSProperties, type ReactNode } from "react";

/**
 * Quiet scroll-entry reveal: 12px rise + fade over 600ms, once. The starting
 * state lives in CSS under `html.js`, so the element is pre-hidden before
 * first paint and simply renders at full opacity if scripts never run.
 */
export default function FadeIn({
  children,
  delay = 0,
  className = "",
}: {
  children: ReactNode;
  delay?: number;
  className?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const io = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            el.classList.add("is-visible");
            io.disconnect();
          }
        }
      },
      { rootMargin: "0px 0px -8% 0px", threshold: 0.05 },
    );

    io.observe(el);
    return () => io.disconnect();
  }, []);

  return (
    <div
      ref={ref}
      className={`fade-in ${className}`.trim()}
      style={{ "--fade-delay": `${delay}ms` } as CSSProperties}
    >
      {children}
    </div>
  );
}
