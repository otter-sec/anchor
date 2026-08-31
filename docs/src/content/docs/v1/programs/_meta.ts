import type { MetaFile } from '@/types'

export default {
  label: 'Program development',
  scrollable: true,
  items: {
    index: { label: 'Overview', order: 0 },
    'account-types': { order: 1 },
    'account-space-and-realloc': { label: 'Account space and realloc', order: 2 },
    errors: { order: 3 },
    events: { order: 4 },
    'zero-copy': { order: 5 },
  },
} satisfies MetaFile
