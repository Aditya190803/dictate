import type { Metadata, Viewport } from "next";
import { Geist, JetBrains_Mono, Newsreader } from "next/font/google";
import "./globals.css";

const geist = Geist({
  variable: "--font-geist",
  subsets: ["latin"],
  display: "swap",
});

const jetbrainsMono = JetBrains_Mono({
  variable: "--font-jetbrains",
  subsets: ["latin"],
  display: "swap",
});

const newsreader = Newsreader({
  variable: "--font-newsreader",
  subsets: ["latin"],
  style: ["normal", "italic"],
  display: "swap",
});

const DESCRIPTION =
  "Press a key, talk, and the words land in the focused window. A daemon-friendly speech-to-text CLI for Wayland Linux and Windows, with realtime transcription and a fully offline mode.";

export const metadata: Metadata = {
  metadataBase: new URL("https://dictate.adityamer.dev"),
  title: "dictate",
  description: DESCRIPTION,
  keywords: [
    "speech-to-text", "dictation", "voice typing", "wayland", "linux",
    "windows", "cli", "whisper", "voxtral", "deepgram", "transcription", "rust",
  ],
  authors: [{ name: "Aditya Mer" }],
  openGraph: {
    title: "dictate",
    description: DESCRIPTION,
    type: "website",
    url: "https://dictate.adityamer.dev",
    siteName: "dictate",
  },
  twitter: {
    card: "summary_large_image",
    title: "dictate",
    description: DESCRIPTION,
  },
};

export const viewport: Viewport = {
  themeColor: "#fcfcfa",
};

/* Marks the document as scripted so the scroll-reveal starting states
   (gated on `html.js` in CSS) apply before first paint. If scripts never
   run, every section simply renders at full opacity. */
const BOOT = `(function(){document.documentElement.classList.add('js')})()`;

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html
      lang="en"
      className={`${geist.variable} ${jetbrainsMono.variable} ${newsreader.variable}`}
      suppressHydrationWarning
    >
      <head>
        <script dangerouslySetInnerHTML={{ __html: BOOT }} />
      </head>
      <body suppressHydrationWarning>{children}</body>
    </html>
  );
}
