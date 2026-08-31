import { defineCollection } from 'astro:content'
import { glob } from 'astro/loaders'
import { z } from 'astro/zod'

const badgeShorthand = z.enum(['new', 'beta', 'deprecated', 'soon'])
const badgeFull = z.object({
  text: z.string(),
  variant: z.enum(['default', 'note', 'tip', 'caution', 'danger']).optional(),
})
const sidebarBadge = z.union([badgeShorthand, badgeFull])

const sidebarConfig = z.object({
  order: z.number().optional(),
  label: z.string().optional(),
  badge: sidebarBadge.optional(),
  hidden: z.boolean().default(false),
})

const navigationLink = z.object({
  label: z.string(),
  link: z.string(),
})

const tableOfContentsConfig = z
  .object({
    minDepth: z.number().int().min(1).max(6).default(2),
    maxDepth: z.number().int().min(1).max(6).default(4),
  })
  .refine((value) => value.minDepth <= value.maxDepth, {
    message: 'tableOfContents.minDepth must be less than or equal to maxDepth',
    path: ['minDepth'],
  })

const docsSchema = z.object({
  title: z.string().max(100),
  description: z.string().max(200).optional(),
  sidebar: sidebarConfig.optional(),
  editUrl: z.union([z.url(), z.literal(false)]).optional(),
  lastUpdated: z.union([z.boolean(), z.coerce.date()]).optional(),
  prev: z.union([navigationLink, z.literal(false)]).optional(),
  next: z.union([navigationLink, z.literal(false)]).optional(),
  tableOfContents: z.union([z.boolean(), tableOfContentsConfig]).optional(),
  banner: z.union([z.string(), z.literal(false)]).optional(),
  wide: z.boolean().default(false),
  pageHeader: z.boolean().default(true),
  autoCards: z.boolean().default(true),
  draft: z.boolean().default(false),
})

const docs = defineCollection({
  loader: glob({ pattern: '**/*.{md,mdx}', base: './src/content/docs' }),
  schema: docsSchema,
})

export const collections = { docs }
