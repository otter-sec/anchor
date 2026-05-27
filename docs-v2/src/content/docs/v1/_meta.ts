import type { MetaFile } from '@/types'

export default {
  label: 'Anchor v1',
  items: {
    index: { label: 'Overview', order: 0 },
    'get-started': { order: 1 },
    fundamentals: { order: 2 },
    programs: { label: 'Program development', order: 3 },
    clients: { order: 4 },
    tokens: { order: 5 },
    testing: { order: 6 },
    security: { label: 'Security and production', order: 7 },
    reference: { order: 8 },
  },
} satisfies MetaFile
