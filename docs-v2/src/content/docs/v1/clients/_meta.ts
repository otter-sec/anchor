import type { MetaFile } from '@/types'

export default {
  label: 'Clients and IDL',
  scrollable: true,
  items: {
    typescript: { order: 0 },
    rust: { order: 1 },
    'declare-program': { label: 'Dependency-free composability', order: 2 },
  },
} satisfies MetaFile
