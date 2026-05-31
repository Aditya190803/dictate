"use client";

import { useEffect, useRef } from "react";

interface ScrollRevealProps {
  children: React.ReactNode;
  className?: string;
  as?: keyof React.JSX.IntrinsicElements;
  stagger?: boolean;
}

export default function ScrollReveal({
  children,
  className = "",
  as: Tag = "div",
  stagger = false,
}: ScrollRevealProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            entry.target.classList.add("visible");
            observer.unobserve(entry.target);
          }
        });
      },
      { threshold: 0.15, rootMargin: "0px 0px -40px 0px" }
    );

    if (stagger) {
      const children = el.querySelectorAll(".reveal");
      children.forEach((child) => observer.observe(child));
    } else {
      observer.observe(el);
    }

    return () => observer.disconnect();
  }, [stagger]);

  const combinedClass = stagger
    ? `reveal-stagger ${className}`.trim()
    : `reveal ${className}`.trim();

  return (
    // @ts-expect-error - dynamic element type
    <Tag ref={ref} className={combinedClass}>
      {children}
    </Tag>
  );
}
