import type { MetaFile } from '@/types'

export default {
  label: 'Reference',
  scrollable: true,
  items: {
    index: { label: 'Overview', order: 0 },
    'macros-and-attributes': { order: 1 },
    'account-constraints': { order: 2 },
    'account-types': { order: 3 },
    'feature-flags': { order: 4 },
    'anchor-toml': { label: 'Anchor.toml', order: 5 },
    cli: { label: 'Anchor CLI', order: 6 },
    'examples-and-benchmarks': { order: 7 },
    'alpha-limitations': { order: 8 },
  },
} satisfies MetaFile
