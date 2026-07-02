import type { DocsConfig, IconMap, Site, SocialLink } from '@/types'

export const SITE: Site = {
  title: 'Anchor Docs',
  description: 'Anchor is the leading development framework for building Solana programs.',
  href: 'https://v2.anchor-lang.com',
  author: 'otter-sec',
  locale: 'en-US',
}

export const SOCIAL_LINKS: SocialLink[] = [
  { href: 'https://github.com/otter-sec/anchor', label: 'GitHub' },
  { href: 'https://discord.com/invite/NHHGSXAnXk', label: 'Discord' },
]

export const ICON_MAP: IconMap = {
  Website: 'world',
  GitHub: 'brand-github',
  LinkedIn: 'brand-linkedin',
  Twitter: 'brand-twitter',
  Email: 'mail',
  RSS: 'rss',
  Discord: 'message-circle',
}

export const DOCS: DocsConfig = {
  repoUrl: 'https://github.com/otter-sec/anchor',
  editUrlBase:
    'https://github.com/otter-sec/anchor/edit/master/docs-v2/src/content/docs/',
  defaultEditUrl: true,
  defaultLastUpdated: true,
  defaultTableOfContents: { minDepth: 2, maxDepth: 4 },
  search: {
    enabled: true,
    hotkey: { mac: '⌘ K', windows: 'Ctrl K' },
  },
  announcement: {
    id: 'v2-alpha',
    message: 'Anchor v2 alpha is here! Up to 95% smaller binaries, 3.0 to 50.4× fewer CU',
    href: '/docs/v2/',
  },
}
