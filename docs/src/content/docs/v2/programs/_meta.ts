import type { MetaFile } from '@/types'

export default {
  label: 'Program development',
  scrollable: true,
  items: {
    index: { label: 'Overview', order: 0 },
    'account-data-model': { order: 1 },
    'account-types': { order: 2 },
    'pod-types': { label: 'Pod types', order: 3 },
    'borsh-accounts-and-realloc': { label: 'Borsh accounts and realloc', order: 4 },
    'errors-and-require': { label: 'Errors and require', order: 5 },
    events: { order: 6 },
    extensibility: { order: 7 },
  },
} satisfies MetaFile
