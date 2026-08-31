import type { MetaFile } from '@/types'

export default {
  label: 'Fundamentals',
  scrollable: true,
  items: {
    index: { label: 'Overview', order: 0 },
    'program-structure': { order: 1 },
    'accounts-and-context': { order: 2 },
    'account-validation': { order: 3 },
    'pdas-and-resolution': { label: 'PDAs and resolution', order: 4 },
    idl: { label: 'IDL', order: 5 },
    cpi: { label: 'Cross-program invocation', order: 6 },
  },
} satisfies MetaFile
