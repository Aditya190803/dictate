"use client";

import { useState, useCallback } from "react";

interface CopyButtonProps {
  text: string;
  label?: string;
  id?: string;
  variant?: "dark" | "light";
}

export default function CopyButton({
  text,
  label = "copy",
  id = "copy-btn",
  variant = "light",
}: CopyButtonProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  }, [text]);

  const cls = variant === "dark" ? "copy-btn" : "copy-btn-light";

  return (
    <button
      className={`${cls}${copied ? " copied" : ""}`}
      onClick={handleCopy}
      aria-label={copied ? "Copied!" : label}
      id={id}
    >
      {copied ? "copied ✓" : label}
    </button>
  );
}
