import type { MetaFile } from '@/types'

export default {
  label: 'Security and production',
  scrollable: true,
  items: {
    'sealevel-attacks': { order: 0 },
    footguns: { order: 1 },
    'verifiable-builds': { order: 2 },
  },
} satisfies MetaFile
