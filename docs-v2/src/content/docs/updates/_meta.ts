import type { MetaFile } from '@/types'

export default {
  label: 'Anchor project updates',
  items: {
    'release-notes': { order: 0 },
    changelog: { order: 1 },
    'contribution-guide': { order: 2 },
    'backport-workflow': { label: 'Backport workflow', order: 3 },
  },
} satisfies MetaFile
