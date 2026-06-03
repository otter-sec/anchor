import "./global.css";
import { Provider } from "./provider";
import { Inter } from "next/font/google";
import type { ReactNode } from "react";
import { GoogleAnalytics } from "@next/third-parties/google";
import { InkeepChatButton } from "./components/inkeep/inkeep-chat-button";
const inter = Inter({
  subsets: ["latin"],
});

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" className={inter.className} suppressHydrationWarning>
      <body className="flex flex-col min-h-screen">
        <Provider>{children}</Provider>
        <InkeepChatButton />
      </body>
      <GoogleAnalytics gaId="G-ZJYNM2WNM0" />
    </html>
  );
}
