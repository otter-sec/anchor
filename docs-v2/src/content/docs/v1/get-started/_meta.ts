import type { MetaFile } from '@/types'

export default {
  label: 'Get started',
  scrollable: true,
  items: {
    installation: { order: 0 },
    'solana-playground': { label: 'Solana Playground', order: 1 },
    'local-development': { label: 'Local development', order: 2 },
  },
} satisfies MetaFile
