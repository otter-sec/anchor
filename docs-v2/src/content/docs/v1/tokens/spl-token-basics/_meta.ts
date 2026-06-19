import type { MetaFile } from '@/types'

export default {
  label: 'SPL token basics',
  items: {
    index: { label: 'Overview', order: 0 },
    'create-mint': { order: 1 },
    'create-token-account': { order: 2 },
    'mint-tokens': { order: 3 },
    'transfer-tokens': { order: 4 },
  },
} satisfies MetaFile
