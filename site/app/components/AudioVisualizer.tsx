"use client";

import { useEffect, useRef, useState, useCallback } from "react";

interface AudioVisualizerProps {
  active?: boolean;
  barCount?: number;
}

export default function AudioVisualizer({
  active = false,
  barCount = 48,
}: AudioVisualizerProps) {
  const [heights, setHeights] = useState<number[]>(
    () => Array.from({ length: barCount }, () => 2)
  );
  const rafRef = useRef<number>(0);
  const activeRef = useRef(active);

  activeRef.current = active;

  const animate = useCallback(() => {
    if (!activeRef.current) {
      setHeights(Array.from({ length: barCount }, () => 2));
      return;
    }

    setHeights((prev) =>
      prev.map((_, i) => {
        const wave = Math.sin(Date.now() * 0.004 + i * 0.3) * 0.5 + 0.5;
        const noise = Math.random() * 0.3;
        const center = Math.abs(i - barCount / 2) / (barCount / 2);
        const envelope = 1 - center * 0.6;
        return Math.max(2, (wave + noise) * 40 * envelope);
      })
    );

    rafRef.current = requestAnimationFrame(animate);
  }, [barCount]);

  useEffect(() => {
    if (active) {
      rafRef.current = requestAnimationFrame(animate);
    } else {
      setHeights(Array.from({ length: barCount }, () => 2));
    }

    return () => {
      if (rafRef.current) cancelAnimationFrame(rafRef.current);
    };
  }, [active, animate, barCount]);

  return (
    <div className="osc" aria-hidden="true">
      {heights.map((h, i) => (
        <span
          key={i}
          className="osc-bar"
          style={{
            height: `${h}px`,
            opacity: active ? 0.4 + (h / 40) * 0.6 : 0.15,
          }}
        />
      ))}
    </div>
  );
}
