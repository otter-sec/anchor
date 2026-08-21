import type { MetaFile } from '@/types'

export default {
  label: 'Fundamentals',
  scrollable: true,
  items: {
    index: { label: 'Overview', order: 0 },
    'program-structure': { order: 1 },
    idl: { label: 'IDL', order: 2 },
    pdas: { label: 'Program derived addresses', order: 3 },
    cpi: { label: 'Cross-program invocation', order: 4 },
  },
} satisfies MetaFile
