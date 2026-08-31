import type { MetaFile } from '@/types'

export default {
  items: {
    index: { label: 'Docs home', order: 0 },
    v1: { label: 'Anchor v1', order: 1, collapsed: true },
    v2: {
      label: 'Anchor v2',
      order: 2,
      badge: { text: 'Alpha', variant: 'note' },
      collapsed: false,
    },
    updates: { order: 3, collapsed: true },
  },
} satisfies MetaFile
