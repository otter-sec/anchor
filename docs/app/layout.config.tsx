import { type DocsLayoutProps } from "fumadocs-ui/layouts/notebook";
import { type HomeLayoutProps } from "fumadocs-ui/layouts/home";
import { docsSource } from "./source";
import StackExchangeIcon from "@/public/icons/stackexchange.svg";
import GithubIcon from "@/public/icons/github.svg";
import DiscordIcon from "@/public/icons/discord.svg";
import Image from "next/image";

/**
 * Shared layout configurations
 *
 * you can configure layouts individually from:
 * Home Layout: app/(home)/layout.tsx
 * Docs Layout: app/docs/layout.tsx
 */
export const baseOptions: HomeLayoutProps = {
  nav: {
    title: (
      <div className="flex items-center gap-2 pl-2">
        <Image src="/icons/anchor.png" alt="Logo" width={24} height={24} />
        <span>Anchor Docs</span>
      </div>
    ),
    url: "/docs",
  },
  links: [
    {
      icon: <GithubIcon />,
      text: "Github",
      url: "https://github.com/otter-sec/anchor",
      active: "none",
    },
    {
      icon: <DiscordIcon />,
      text: "Discord",
      url: "https://discord.com/invite/NHHGSXAnXk",
      active: "none",
    },
    {
      icon: <StackExchangeIcon />,
      text: "Stack Exchange",
      url: "https://solana.stackexchange.com/",
      active: "none",
    },
  ],
};

export const docsOptions: DocsLayoutProps = {
  ...baseOptions,
  sidebar: {
    defaultOpenLevel: 1,
  },
  tree: docsSource.pageTree,
};
