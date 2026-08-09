"use client";

import { useEffect, useRef, type ReactNode, type ElementType } from "react";

/**
 * Scroll reveal via IntersectionObserver + a CSS transition.
 *
 * This replaces GSAP + ScrollTrigger, which were pulling ~100KB to animate
 * opacity and translateY. Reduced-motion is handled in CSS rather than here,
 * so the element is visible even if this never hydrates.
 */
export default function Reveal({
  children,
  as: Tag = "div",
  delay = 0,
  className = "",
  ...rest
}: {
  children: ReactNode;
  as?: ElementType;
  delay?: number;
  className?: string;
  [key: string]: unknown;
}) {
  const ref = useRef<HTMLElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    // No observer (or reduced motion) → show immediately rather than never.
    if (
      typeof IntersectionObserver === "undefined" ||
      window.matchMedia("(prefers-reduced-motion: reduce)").matches
    ) {
      el.dataset.shown = "true";
      return;
    }

    const io = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            (entry.target as HTMLElement).dataset.shown = "true";
            io.unobserve(entry.target);
          }
        }
      },
      { rootMargin: "0px 0px -12% 0px", threshold: 0.05 },
    );

    io.observe(el);
    return () => io.disconnect();
  }, []);

  return (
    <Tag
      ref={ref}
      className={`reveal ${className}`.trim()}
      style={{ "--reveal-delay": `${delay}ms` } as React.CSSProperties}
      {...rest}
    >
      {children}
    </Tag>
  );
}
