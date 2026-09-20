"use client";

import { useState, useCallback } from "react";
import { Check, Copy } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

interface CopyButtonProps {
  text: string;
  label?: string;
  className?: string;
}

export default function CopyButton({
  text,
  label = "Copy",
  className,
}: CopyButtonProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [text]);

  return (
    <button
      type="button"
      onClick={handleCopy}
      aria-label={copied ? "Copied" : label}
      className={cn(
        "inline-flex h-8 shrink-0 cursor-pointer items-center gap-1.5 rounded-md border px-2.5 font-mono text-xs transition-colors",
        copied
          ? "border-go/40 bg-go-soft text-go-ink"
          : "text-muted-foreground hover:bg-secondary hover:text-foreground",
        className,
      )}
    >
      {copied ? <Check weight="bold" className="size-3.5" /> : <Copy className="size-3.5" />}
      {copied ? "Copied" : label}
    </button>
  );
}
