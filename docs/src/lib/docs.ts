import { DOCS } from '@/consts'
import { getCollection, type CollectionEntry } from 'astro:content'

export type Doc = CollectionEntry<'docs'>

export async function getAllDocs(): Promise<Doc[]> {
  const docs = await getCollection('docs')
  return docs.filter((doc) => !doc.data.draft)
}

// Astro injects `BASE_URL` from the `base:` field in astro.config.ts.
// Default to `/` for tools that import this file outside an Astro build.
export const BASE_URL: string = (import.meta.env?.BASE_URL ?? '/').replace(/\/?$/, '/')

/**
 * Prefix a root-relative path with the configured `base`.
 * Use for hardcoded internal links, asset hrefs, and image srcs in Astro
 * files. Idempotent if the input already starts with `BASE_URL`.
 */
export function withBase(path: string): string {
  if (path.startsWith(BASE_URL)) return path
  return BASE_URL + path.replace(/^\//, '')
}

export function docHref(id: string): string {
  if (id === 'index') return BASE_URL
  if (id.endsWith('/index')) return BASE_URL + id.slice(0, -'/index'.length) + '/'
  return BASE_URL + id + '/'
}

export function docOgImageHref(id: string): string {
  const slug = docSlugFromId(id) ?? 'index'
  return `${BASE_URL}og/${slug}.png`
}

export function docSlugFromId(id: string): string | undefined {
  if (id === 'index') return undefined
  if (id.endsWith('/index')) return id.slice(0, -'/index'.length)
  return id
}

export function docLabel(doc: Doc): string {
  return doc.data.sidebar?.label ?? doc.data.title
}

export function getEditUrl(doc: Doc): string | null {
  if (doc.data.editUrl === false) return null
  if (typeof doc.data.editUrl === 'string') return doc.data.editUrl
  if (!DOCS.defaultEditUrl) return null
  if (!DOCS.editUrlBase) return null
  const filePath = doc.filePath
  if (!filePath) return null
  const normalizedPath = filePath.replaceAll('\\', '/')
  const marker = 'src/content/docs/'
  const markerIndex = normalizedPath.indexOf(marker)
  const rel =
    markerIndex === -1 ? normalizedPath : normalizedPath.slice(markerIndex + marker.length)
  return DOCS.editUrlBase.replace(/\/+$/, '') + '/' + rel
}

export function resolveLastUpdated(doc: Doc): Date | null {
  const value = doc.data.lastUpdated
  if (value === false) return null
  if (value instanceof Date) return value
  if (!DOCS.defaultLastUpdated && value !== true) return null
  const injected = (doc.data as { _lastUpdated?: unknown })._lastUpdated
  return injected instanceof Date ? injected : null
}

export function resolveTOC(doc: Doc): {
  enabled: boolean
  minDepth: number
  maxDepth: number
} {
  const cfg = doc.data.tableOfContents
  const defaults = DOCS.defaultTableOfContents
  if (cfg === false) return { enabled: false, ...defaults }
  if (cfg === true || cfg === undefined) return { enabled: true, ...defaults }
  return {
    enabled: true,
    minDepth: cfg.minDepth ?? defaults.minDepth,
    maxDepth: cfg.maxDepth ?? defaults.maxDepth,
  }
}
