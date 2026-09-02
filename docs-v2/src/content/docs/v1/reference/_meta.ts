import type { MetaFile } from '@/types'

export default {
  label: 'Reference',
  scrollable: true,
  items: {
    index: { label: 'Overview', order: 0 },
    'account-constraints': { order: 1 },
    'anchor-toml': { order: 2 },
    cli: { label: 'Anchor CLI', order: 3 },
    avm: { label: 'Anchor version manager', order: 4 },
    'rust-to-js-types': { label: 'Rust to JS type conversion', order: 5 },
    examples: { order: 6 },
  },
} satisfies MetaFile
