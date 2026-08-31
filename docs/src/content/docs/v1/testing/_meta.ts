import type { MetaFile } from '@/types'

export default {
  label: 'Testing and debugging',
  scrollable: true,
  items: {
    index: { label: 'Overview', order: 0 },
    litesvm: { order: 1 },
    mollusk: { order: 2 },
  },
} satisfies MetaFile
