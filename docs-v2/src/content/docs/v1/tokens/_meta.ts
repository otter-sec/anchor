import type { MetaFile } from '@/types'

export default {
  label: 'Tokens and CPI',
  scrollable: true,
  items: {
    index: { label: 'Overview', order: 0 },
    'spl-token-basics': { label: 'SPL token basics', order: 1 },
    'token-2022-and-extensions': { label: 'Token-2022 and extensions', order: 2 },
  },
} satisfies MetaFile
