"use client";

import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import CopyButton from "./CopyButton";
import { useIsWindows } from "../lib/client-hooks";

type Platform = "linux" | "windows";

const INSTALL: Record<Platform, { label: string; cmd: string; note: React.ReactNode }> = {
  linux: {
    label: "Linux",
    cmd: "curl -fsSL https://dictate.adityamer.dev/install.sh | sh",
    note: (
      <>
        Detects your distro, installs PipeWire deps, and pulls the latest
        release binary. Then run <code className="font-mono text-[0.85em]">dictate setup</code>.
      </>
    ),
  },
  windows: {
    label: "Windows",
    cmd: "irm https://dictate.adityamer.dev/install.ps1 | iex",
    note: (
      <>
        Downloads the latest release binary (or builds from source), adds
        dictate to PATH, then run <code className="font-mono text-[0.85em]">dictate setup</code>.
        No C compiler needed — TLS uses SChannel.
      </>
    ),
  },
};

function CommandRow({ cmd }: { cmd: string }) {
  return (
    <div className="flex items-center gap-3 rounded-lg border bg-card py-3 pr-3 pl-4">
      <code className="flex-1 overflow-x-auto font-mono text-[0.83rem] whitespace-nowrap text-foreground [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        <span className="mr-2.5 text-go select-none" aria-hidden="true">
          $
        </span>
        {cmd}
      </code>
      <CopyButton text={cmd} />
    </div>
  );
}

export default function InstallTabs() {
  const detected: Platform = useIsWindows() ? "windows" : "linux";

  return (
    <Tabs defaultValue={detected} key={detected}>
      <TabsList>
        {(Object.keys(INSTALL) as Platform[]).map((p) => (
          <TabsTrigger key={p} value={p}>
            {INSTALL[p].label}
          </TabsTrigger>
        ))}
      </TabsList>
      {(Object.keys(INSTALL) as Platform[]).map((p) => (
        <TabsContent key={p} value={p}>
          <CommandRow cmd={INSTALL[p].cmd} />
          <p className="mt-3 max-w-[62ch] text-sm text-muted-foreground">
            {INSTALL[p].note}
          </p>
        </TabsContent>
      ))}
    </Tabs>
  );
}
