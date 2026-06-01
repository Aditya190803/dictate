import type { Metadata } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";
import "./globals.css";

const inter = Inter({
  variable: "--font-inter",
  subsets: ["latin"],
  display: "swap",
});

const jetbrainsMono = JetBrains_Mono({
  variable: "--font-jetbrains",
  subsets: ["latin"],
  display: "swap",
});

export const metadata: Metadata = {
  title: "dictate — Wayland Speech-to-Text",
  description:
    "Press a keybind and get instant text output. A signal-driven CLI for Linux with Mistral realtime STT by default and BATCH_MODE as an opt-out.",
  keywords: [
    "speech-to-text", "wayland", "linux", "cli", "whisper",
    "dictation", "voice-to-text", "transcription", "rust", "pipewire",
  ],
  openGraph: {
    title: "dictate — Wayland Speech-to-Text",
    description:
      "Press a keybind, speak, get text. Mistral realtime STT by default for Wayland Linux desktops.",
    type: "website",
    url: "https://dictate.adityamer.dev",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="en"
      className={`${inter.variable} ${jetbrainsMono.variable}`}
      suppressHydrationWarning
    >
      <body
        style={{ minHeight: "100vh", display: "flex", flexDirection: "column" }}
        suppressHydrationWarning
      >
        {children}
      </body>
    </html>
  );
}
