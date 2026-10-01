import type { MetaFile } from '@/types'

export default {
  label: 'Reference',
  scrollable: true,
  items: {
    index: { label: 'Overview', order: 0 },
    'account-constraints': { order: 1 },
    'anchor-toml': { order: 2 },
    cli: { label: 'Anchor CLI', order: 3 },
    'no-dna': { label: 'NO_DNA', order: 4 },
    avm: { label: 'Anchor version manager', order: 5 },
    'rust-to-js-types': { label: 'Rust to JS type conversion', order: 6 },
    examples: { order: 7 },
  },
} satisfies MetaFile
