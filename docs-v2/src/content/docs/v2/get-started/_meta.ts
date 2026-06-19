import type { MetaFile } from '@/types'

export default {
  label: 'Get started',
  scrollable: true,
  items: {
    installation: { order: 0 },
    'first-program': { label: 'First program', order: 1 },
    'migrating-from-v1': { label: 'Migrating from v1', order: 2 },
  },
} satisfies MetaFile
