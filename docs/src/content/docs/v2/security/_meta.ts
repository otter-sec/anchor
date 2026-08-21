import type { MetaFile } from '@/types'

export default {
  label: 'Security and production',
  scrollable: true,
  items: {
    'secure-by-default': { order: 0 },
    'production-builds': { order: 1 },
    'performance-and-optimizations': { order: 2 },
    'custom-entrypoint': { order: 3 },
    'raw-account-access': { order: 4 },
  },
} satisfies MetaFile
