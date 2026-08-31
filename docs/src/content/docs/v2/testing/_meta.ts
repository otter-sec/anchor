import type { MetaFile } from '@/types'

export default {
  label: 'Testing and debugging',
  scrollable: true,
  items: {
    index: { label: 'Overview', order: 0 },
    litesvm: { label: 'LiteSVM', order: 1 },
    'profiling-and-debugger': { order: 2 },
    coverage: { order: 3 },
  },
} satisfies MetaFile
