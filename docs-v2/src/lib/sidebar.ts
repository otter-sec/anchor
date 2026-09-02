import { BASE_URL, docHref, docLabel, getAllDocs, type Doc } from '@/lib/docs'
import {
  candidateDocIdsForVersion,
  DOCS_VERSION_LABELS,
  type DocsVersion,
} from '@/lib/docs-versions'
import { isCurrentPath, titleCase, trimTrailingSlash } from '@/lib/utils'
import type {
  FlatDoc,
  MetaFile,
  MetaItemOverride,
  SidebarBadge,
  SidebarGroup,
  SidebarLink,
  SidebarNode,
} from '@/types'

const metaModules = {
  ...(import.meta.glob('/src/content/docs/_meta.ts', {
    eager: true,
  }) as Record<string, { default: MetaFile }>),
  ...(import.meta.glob('/src/content/docs/**/_meta.ts', {
    eager: true,
  }) as Record<string, { default: MetaFile }>),
}

function metaFor(dirPath: string): MetaFile {
  const key = dirPath ? `/src/content/docs/${dirPath}/_meta.ts` : '/src/content/docs/_meta.ts'
  return metaModules[key]?.default ?? {}
}

type TreeNode = {
  name: string
  fullPath: string
  doc?: Doc
  children: TreeNode[]
}

type DocTreeNode = TreeNode & {
  doc: Doc
}

function hasDoc(node: TreeNode): node is DocTreeNode {
  return Boolean(node.doc)
}

function ensureNode(parent: TreeNode, name: string, fullPath: string): TreeNode {
  let existing = parent.children.find((c) => c.name === name)
  if (!existing) {
    existing = { name, fullPath, children: [] }
    parent.children.push(existing)
  }
  return existing
}

function buildTree(docs: Doc[]): TreeNode {
  const root: TreeNode = { name: '', fullPath: '', children: [] }

  for (const doc of docs) {
    const parts = doc.id.split('/')
    let node = root
    for (let i = 0; i < parts.length - 1; i++) {
      const dirPath = parts.slice(0, i + 1).join('/')
      node = ensureNode(node, parts[i], dirPath)
    }
    const lastName = parts[parts.length - 1]
    const leaf = ensureNode(node, lastName, doc.id)
    leaf.doc = doc
  }

  return root
}

export type SidebarContext = {
  docs: Doc[]
  tree: TreeNode
  docByHref: Map<string, Doc>
  rootMeta: MetaFile
}

export function createSidebarContext(docs: Doc[]): SidebarContext {
  return {
    docs,
    tree: buildTree(docs),
    docByHref: new Map(docs.map((doc) => [docHref(doc.id), doc])),
    rootMeta: metaFor(''),
  }
}

let sidebarContextPromise: Promise<SidebarContext> | null = null

export function loadSidebarContext(): Promise<SidebarContext> {
  if (!sidebarContextPromise) {
    sidebarContextPromise = getAllDocs().then(createSidebarContext)
  }
  return sidebarContextPromise
}

type OrderKey = {
  order: number
  label: string
}

function compareByOrder(a: OrderKey, b: OrderKey): number {
  if (a.order !== b.order) return a.order - b.order
  return a.label.localeCompare(b.label)
}

function resolveDocLink(
  node: DocTreeNode,
  parentMeta: MetaFile,
  pathname: string,
): { link: SidebarLink; sortKey: OrderKey } | null {
  const { doc } = node
  if (doc.data.sidebar?.hidden) return null

  const override: MetaItemOverride = parentMeta.items?.[node.name] ?? {}
  if (override.hidden) return null

  const href = docHref(doc.id)
  const label = doc.data.sidebar?.label ?? override.label ?? docLabel(doc)
  const order = doc.data.sidebar?.order ?? override.order ?? Infinity
  const badge: SidebarBadge | undefined = doc.data.sidebar?.badge ?? override.badge

  return {
    link: {
      type: 'link',
      label,
      href,
      badge,
      isCurrent: isCurrentPath(href, pathname),
    },
    sortKey: { order, label },
  }
}

function resolveGroup(
  node: TreeNode,
  parentMeta: MetaFile,
  pathname: string,
): { group: SidebarGroup; sortKey: OrderKey } | null {
  const own = metaFor(node.fullPath)
  if (own.hidden) return null

  const override: MetaItemOverride = parentMeta.items?.[node.name] ?? {}
  if (override.hidden) return null

  const items = buildNodes(node, own, pathname)

  // If the group's directory has an index doc, hoist its href/badge onto the
  // group itself so the summary row becomes a real link to that page,
  // instead of injecting a separate "Overview" child entry.
  const indexDoc =
    node.doc && !node.doc.data.sidebar?.hidden && !(own.items?.index?.hidden ?? false)
      ? node.doc
      : null
  const groupHref = indexDoc ? docHref(indexDoc.id) : undefined
  const groupIsCurrent = groupHref ? isCurrentPath(groupHref, pathname) : false

  if (items.length === 0 && !indexDoc) return null

  const label = own.label ?? override.label ?? titleCase(node.name)
  const order = override.order ?? own.order ?? Infinity
  const badge: SidebarBadge | undefined =
    indexDoc?.data.sidebar?.badge ?? override.badge ?? own.badge
  const forceOpen = override.forceOpen ?? own.forceOpen ?? false

  const hasActiveDescendant =
    groupIsCurrent ||
    items.some(
      (i) => (i.type === 'link' && i.isCurrent) || (i.type === 'group' && i.hasActiveDescendant),
    )

  return {
    group: {
      type: 'group',
      label,
      collapsed: forceOpen ? false : (override.collapsed ?? own.collapsed ?? !hasActiveDescendant),
      forceOpen,
      badge,
      hasActiveDescendant,
      href: groupHref,
      isCurrent: groupIsCurrent,
      items,
    },
    sortKey: { order, label },
  }
}

function buildNodes(parent: TreeNode, parentMeta: MetaFile, pathname: string): SidebarNode[] {
  const resolved: Array<{ node: SidebarNode; sortKey: OrderKey }> = []

  for (const child of parent.children) {
    const isRootIndex = child.fullPath === 'index' && !!child.doc
    if (isRootIndex) continue

    if (child.children.length > 0) {
      const result = resolveGroup(child, parentMeta, pathname)
      if (result) resolved.push({ node: result.group, sortKey: result.sortKey })
    } else if (hasDoc(child)) {
      const result = resolveDocLink(child, parentMeta, pathname)
      if (result) resolved.push({ node: result.link, sortKey: result.sortKey })
    }
  }

  resolved.sort((a, b) => compareByOrder(a.sortKey, b.sortKey))
  return resolved.map((r) => r.node)
}

function urlForPath(pathname: string): URL {
  try {
    return new URL(pathname, 'https://anchor.local')
  } catch {
    return new URL('/', 'https://anchor.local')
  }
}

function pathnameWithinBase(pathname: string): string {
  let path = urlForPath(pathname).pathname

  if (BASE_URL !== '/' && path.startsWith(BASE_URL)) {
    path = path.slice(BASE_URL.length)
  } else {
    path = path.replace(/^\//, '')
  }

  return path.replace(/^\/+|\/+$/g, '')
}

function searchDocsVersion(pathname: string): DocsVersion | null {
  const version = urlForPath(pathname).searchParams.get('version')
  return version === 'v1' || version === 'v2' ? version : null
}

export function getActiveDocsVersion(pathname: string): DocsVersion | null {
  const [section] = pathnameWithinBase(pathname).split('/')
  return section === 'v1' || section === 'v2' ? section : null
}

function getFocusedDocsVersion(pathname: string): DocsVersion | null {
  const active = getActiveDocsVersion(pathname)
  if (active) return active

  const section = pathnameWithinBase(pathname).split('/')[0]
  if (section === 'updates') return searchDocsVersion(pathname) ?? 'v2'
  return null
}

function findChild(parent: TreeNode, name: string): TreeNode | undefined {
  return parent.children.find((child) => child.name === name)
}

function getVersionScopedNodes(
  tree: TreeNode,
  version: DocsVersion,
  rootMeta: MetaFile,
  pathname: string,
): SidebarNode[] {
  const versionNode = findChild(tree, version)
  const versionMeta = metaFor(version)
  const versionItems = versionNode ? buildNodes(versionNode, versionMeta, pathname) : []

  const updatesNode = findChild(tree, 'updates')
  const updatesGroup = updatesNode ? resolveGroup(updatesNode, rootMeta, pathname)?.group : null

  return updatesGroup ? [...versionItems, updatesGroup] : versionItems
}

export function getSidebarTreeFromContext(
  context: SidebarContext,
  pathname: string = '/',
): SidebarNode[] {
  const version = getFocusedDocsVersion(pathname)
  if (version) return getVersionScopedNodes(context.tree, version, context.rootMeta, pathname)
  return buildNodes(context.tree, context.rootMeta, pathname)
}

export function getVersionSidebarsFromContext(
  context: SidebarContext,
  pathname: string,
): { v1: SidebarNode[]; v2: SidebarNode[] } {
  return {
    v1: getVersionScopedNodes(context.tree, 'v1', context.rootMeta, pathname),
    v2: getVersionScopedNodes(context.tree, 'v2', context.rootMeta, pathname),
  }
}

export async function getSidebarTree(pathname: string = '/'): Promise<SidebarNode[]> {
  return getSidebarTreeFromContext(await loadSidebarContext(), pathname)
}

function flattenTree(nodes: SidebarNode[], acc: FlatDoc[] = []): FlatDoc[] {
  for (const node of nodes) {
    if (node.type === 'link') {
      acc.push({
        id: '',
        href: node.href,
        title: node.label,
        label: node.label,
        hidden: false,
      })
    } else {
      if (node.href) {
        acc.push({
          id: '',
          href: node.href,
          title: node.label,
          label: node.label,
          hidden: false,
        })
      }
      flattenTree(node.items, acc)
    }
  }
  return acc
}

export function getFlatDocOrderFromContext(
  context: SidebarContext,
  pathname: string = '/',
): FlatDoc[] {
  return flattenTree(getSidebarTreeFromContext(context, pathname))
}

export async function getFlatDocOrder(pathname: string = '/'): Promise<FlatDoc[]> {
  return getFlatDocOrderFromContext(await loadSidebarContext(), pathname)
}

export type ScrollGroupPage = {
  doc: Doc
  href: string
  label: string
}

export type ScrollableDocGroup = {
  path: string
  label: string
  description?: string
  pages: ScrollGroupPage[]
}

function parentPath(path: string): string {
  const parts = path.split('/')
  return parts.slice(0, -1).join('/')
}

function ownName(path: string): string {
  const parts = path.split('/')
  return parts[parts.length - 1] ?? path
}

function groupOverride(path: string): MetaItemOverride {
  const parent = parentPath(path)
  return metaFor(parent).items?.[ownName(path)] ?? {}
}

function groupLabel(path: string): string {
  const own = metaFor(path)
  const override = groupOverride(path)
  return own.label ?? override.label ?? titleCase(ownName(path))
}

function isScrollableGroup(path: string): boolean {
  const own = metaFor(path)
  const override = groupOverride(path)
  return own.scrollable ?? override.scrollable ?? false
}

function docGroupAncestors(doc: Doc): string[] {
  if (doc.id === 'index') return []

  const parts = doc.id.split('/')
  const isIndexDoc = /[/\\]index\.mdx?$/.test(doc.filePath ?? '')
  const dirs = isIndexDoc ? parts : parts.slice(0, -1)

  const paths: string[] = []

  for (let length = dirs.length; length > 0; length--) {
    paths.push(dirs.slice(0, length).join('/'))
  }

  return paths
}

function docBelongsToGroup(doc: Doc, groupPath: string): boolean {
  return (
    doc.id === groupPath || doc.id === `${groupPath}/index` || doc.id.startsWith(`${groupPath}/`)
  )
}

function groupPagesFromFlatOrder(
  context: SidebarContext,
  groupPath: string,
  pathname: string,
): ScrollGroupPage[] {
  return getFlatDocOrderFromContext(context, pathname).flatMap((page) => {
    const doc = context.docByHref.get(page.href)
    if (!doc || !docBelongsToGroup(doc, groupPath)) return []
    return [{ doc, href: page.href, label: page.label }]
  })
}

export function getScrollableGroupFromContext(
  context: SidebarContext,
  doc: Doc,
  pathname: string,
): ScrollableDocGroup | null {
  const scrollableGroups = docGroupAncestors(doc).filter(isScrollableGroup)
  const groupPath = scrollableGroups[scrollableGroups.length - 1]
  if (!groupPath) return null

  const pages = groupPagesFromFlatOrder(context, groupPath, pathname)
  if (pages.length <= 1) return null

  const overview = pages.find(
    (page) => page.doc.id === groupPath || page.doc.id === `${groupPath}/index`,
  )
  const group: ScrollableDocGroup = {
    path: groupPath,
    label: groupLabel(groupPath),
    pages,
  }

  if (overview?.doc.data.description) group.description = overview.doc.data.description

  return group
}

export type IndexChild = {
  label: string
  href: string
  description?: string
}

function findGroupChildren(nodes: SidebarNode[], href: string): SidebarNode[] | null {
  const normalized = trimTrailingSlash(href)
  for (const node of nodes) {
    if (node.type !== 'group') continue
    if (node.href && trimTrailingSlash(node.href) === normalized) return node.items
    const found = findGroupChildren(node.items, href)
    if (found !== null) return found
  }
  return null
}

export function getIndexChildrenFromContext(context: SidebarContext, href: string): IndexChild[] {
  const tree = getSidebarTreeFromContext(context, href)
  const nodes = href === BASE_URL ? tree : (findGroupChildren(tree, href) ?? [])

  return nodes.flatMap((node): IndexChild[] => {
    if (node.type === 'link') {
      const doc = context.docByHref.get(node.href)
      return [{ label: node.label, href: node.href, description: doc?.data.description }]
    }
    // For groups, prefer the group's own index doc; fall back to the first
    // descendant link.
    const targetHref =
      node.href ?? (node.items[0]?.type === 'link' ? node.items[0].href : undefined)
    if (!targetHref) return []
    const doc = context.docByHref.get(targetHref)
    return [{ label: node.label, href: targetHref, description: doc?.data.description }]
  })
}

export async function getIndexChildren(href: string): Promise<IndexChild[]> {
  return getIndexChildrenFromContext(await loadSidebarContext(), href)
}

export function getPrevNextFromContext(
  context: SidebarContext,
  currentHref: string,
): { prev: FlatDoc | null; next: FlatDoc | null } {
  const flat = getFlatDocOrderFromContext(context, currentHref)
  const normalized = trimTrailingSlash(currentHref)
  const index = flat.findIndex((d) => trimTrailingSlash(d.href) === normalized)
  if (index === -1) {
    if (normalized === trimTrailingSlash(BASE_URL)) {
      return { prev: null, next: flat[0] ?? null }
    }
    return { prev: null, next: null }
  }
  return {
    prev: index > 0 ? flat[index - 1] : null,
    next: index < flat.length - 1 ? flat[index + 1] : null,
  }
}

export async function getPrevNext(
  currentHref: string,
): Promise<{ prev: FlatDoc | null; next: FlatDoc | null }> {
  return getPrevNextFromContext(await loadSidebarContext(), currentHref)
}

export function getScrollGroupPrevNextFromContext(
  context: SidebarContext,
  group: ScrollableDocGroup,
): { prev: FlatDoc | null; next: FlatDoc | null } {
  if (group.pages.length === 0) return { prev: null, next: null }
  const firstHref = group.pages[0].href
  const lastHref = group.pages[group.pages.length - 1].href
  const { prev } = getPrevNextFromContext(context, firstHref)
  const { next } = getPrevNextFromContext(context, lastHref)
  return { prev, next }
}

export type DocsVersionSwitchOption = {
  version: DocsVersion
  label: string
  href: string
  active: boolean
  badge?: SidebarBadge
}

function fallbackVersionHref(version: DocsVersion): string {
  return docHref(`${version}/index`)
}

function matchingVersionHref(docs: Doc[], version: DocsVersion, relativePath: string): string {
  const ids = new Set(docs.map((doc) => doc.id))
  const candidates = candidateDocIdsForVersion(version, relativePath)
  const match = candidates.find((id) => ids.has(id))
  return match ? docHref(match) : fallbackVersionHref(version)
}

function hrefWithVersionSearch(pathname: string, version: DocsVersion): string {
  const url = urlForPath(pathname)
  url.searchParams.set('version', version)
  return `${url.pathname}${url.search}`
}

export function getDocsVersionSwitchOptionsFromContext(
  context: SidebarContext,
  pathname: string,
): DocsVersionSwitchOption[] {
  const relative = pathnameWithinBase(pathname)
  const parts = relative.split('/').filter(Boolean)
  const active = getFocusedDocsVersion(pathname)
  const isUpdatesPath = parts[0] === 'updates'
  const sameVersionPath = parts[0] === 'v1' || parts[0] === 'v2' ? parts.slice(1).join('/') : ''

  return [
    {
      version: 'v2',
      label: DOCS_VERSION_LABELS.v2,
      href: isUpdatesPath
        ? hrefWithVersionSearch(pathname, 'v2')
        : matchingVersionHref(context.docs, 'v2', sameVersionPath),
      active: active === 'v2',
    },
    {
      version: 'v1',
      label: DOCS_VERSION_LABELS.v1,
      href: isUpdatesPath
        ? hrefWithVersionSearch(pathname, 'v1')
        : matchingVersionHref(context.docs, 'v1', sameVersionPath),
      active: active === 'v1',
    },
  ]
}

export async function getDocsVersionSwitchOptions(
  pathname: string,
): Promise<DocsVersionSwitchOption[]> {
  return getDocsVersionSwitchOptionsFromContext(await loadSidebarContext(), pathname)
}
