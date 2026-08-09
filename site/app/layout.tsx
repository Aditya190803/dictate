import type { Metadata, Viewport } from "next";
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

const DESCRIPTION =
  "Press a key, talk, and the words land in the focused window. A daemon-friendly speech-to-text CLI for Wayland Linux and Windows, with realtime transcription and a fully offline mode.";

export const metadata: Metadata = {
  metadataBase: new URL("https://dictate.adityamer.dev"),
  title: "dictate — speech to text for Linux and Windows",
  description: DESCRIPTION,
  keywords: [
    "speech-to-text", "dictation", "voice typing", "wayland", "linux",
    "windows", "cli", "whisper", "voxtral", "deepgram", "transcription", "rust",
  ],
  authors: [{ name: "Aditya Mer" }],
  openGraph: {
    title: "dictate — speech to text for Linux and Windows",
    description: DESCRIPTION,
    type: "website",
    url: "https://dictate.adityamer.dev",
    siteName: "dictate",
  },
  twitter: {
    card: "summary_large_image",
    title: "dictate — speech to text for Linux and Windows",
    description: DESCRIPTION,
  },
};

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#f5f3ef" },
    { media: "(prefers-color-scheme: dark)", color: "#101211" },
  ],
};

/**
 * Applies a stored theme before first paint. Without this the page renders in
 * the system theme and then snaps to the stored one — a visible flash on every
 * load for anyone who picked the non-default.
 */
const THEME_INIT = `(function(){try{var t=localStorage.getItem('theme');if(t==='dark'||t==='light'){document.documentElement.dataset.theme=t}}catch(e){}})()`;

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html
      lang="en"
      className={`${inter.variable} ${jetbrainsMono.variable}`}
      suppressHydrationWarning
    >
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_INIT }} />
      </head>
      <body suppressHydrationWarning>{children}</body>
    </html>
  );
}
