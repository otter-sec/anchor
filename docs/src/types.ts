export type Site = {
  title: string
  description: string
  href: string
  author: string
  locale: string
}

export type SocialLink = {
  href: string
  label: string
}

export type IconMap = {
  [key: string]: string
}

export type SidebarBadgeVariant = 'default' | 'note' | 'tip' | 'caution' | 'danger'

export type SidebarBadgeShorthand = 'new' | 'beta' | 'deprecated' | 'soon'

export type SidebarBadge =
  | SidebarBadgeShorthand
  | {
      text: string
      variant?: SidebarBadgeVariant
    }

export type MetaItemOverride = {
  label?: string
  order?: number
  badge?: SidebarBadge
  hidden?: boolean
  collapsed?: boolean
  forceOpen?: boolean
  scrollable?: boolean
}

export type MetaFile = {
  label?: string
  order?: number
  collapsed?: boolean
  forceOpen?: boolean
  badge?: SidebarBadge
  hidden?: boolean
  scrollable?: boolean
  items?: Record<string, MetaItemOverride>
}

export type DocsConfig = {
  repoUrl?: string
  editUrlBase?: string | false
  defaultEditUrl: boolean
  defaultLastUpdated: boolean
  defaultTableOfContents: { minDepth: number; maxDepth: number }
  search: {
    enabled: boolean
    hotkey: { mac: string; windows: string }
  }
  announcement: {
    id: string
    message: string
    href?: string
  } | null
}

export type SidebarLink = {
  type: 'link'
  label: string
  href: string
  badge?: SidebarBadge
  isCurrent?: boolean
  isActive?: boolean
}

export type SidebarGroup = {
  type: 'group'
  label: string
  collapsed: boolean
  forceOpen: boolean
  badge?: SidebarBadge
  hasActiveDescendant?: boolean
  /** Present when the group has an index doc - the summary acts as a link. */
  href?: string
  isCurrent?: boolean
  items: SidebarNode[]
}

export type SidebarNode = SidebarLink | SidebarGroup

export type FlatDoc = {
  id: string
  href: string
  title: string
  label: string
  hidden: boolean
}
