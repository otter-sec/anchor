import { navigate } from 'astro:transitions/client'
import { isElementHidden, trimTrailingSlash, withDatasetFlag } from './dom'
import { mountClientModule } from './lifecycle'
import { readJsonRecord, readNumber, removeStorage, writeJson, writeStorage } from './storage'

const SIDEBAR_GROUP_STATE_KEY = 'anchor-docs-sidebar-groups'
const SIDEBAR_SCROLL_STATE_KEY = 'anchor-docs-sidebar-scroll'
const SIDEBAR_PENDING_SCROLL_KEY = 'anchor-docs-sidebar-pending-scroll'
const SIDEBAR_VIEWPORT_SELECTOR =
  'nav[aria-label="Documentation"] [data-slot="scroll-area-viewport"]'

let syncingSidebarGroups = false

const sidebarGroupsWithPersistence = new WeakSet<HTMLDetailsElement>()
const sidebarViewportsWithPersistence = new WeakSet<HTMLElement>()

function readSidebarGroupState(): Record<string, boolean> {
  return readJsonRecord<boolean>(localStorage, SIDEBAR_GROUP_STATE_KEY)
}

function writeSidebarGroupState(state: Record<string, boolean>): void {
  writeJson(localStorage, SIDEBAR_GROUP_STATE_KEY, state)
}

function readSidebarScrollState(): Record<string, number> {
  return readJsonRecord<number>(localStorage, SIDEBAR_SCROLL_STATE_KEY)
}

function writeSidebarScrollState(state: Record<string, number>): void {
  writeJson(localStorage, SIDEBAR_SCROLL_STATE_KEY, state)
}

function sidebarGroupKey(group: HTMLElement): string | null {
  const key = group.dataset.groupPath
  return key && key.length > 0 ? key : null
}

function matchingSidebarGroups(key: string): NodeListOf<HTMLDetailsElement> {
  return document.querySelectorAll<HTMLDetailsElement>(
    `details[data-sidebar-group][data-group-path="${CSS.escape(key)}"]`,
  )
}

function setMatchingSidebarGroups(key: string, open: boolean, source: HTMLDetailsElement): void {
  syncingSidebarGroups = true

  matchingSidebarGroups(key).forEach((group) => {
    if (group !== source) group.open = open
  })

  syncingSidebarGroups = false
}

function restoreSidebarGroups(): void {
  const state = readSidebarGroupState()

  document
    .querySelectorAll<HTMLDetailsElement>('details[data-sidebar-group][data-group-path]')
    .forEach((group) => {
      const key = sidebarGroupKey(group)
      if (!key || state[key] === undefined) return
      group.open = state[key]
    })
}

function setupPersistentSidebarGroups(): void {
  restoreSidebarGroups()

  document
    .querySelectorAll<HTMLDetailsElement>('details[data-sidebar-group][data-group-path]')
    .forEach((group) => {
      if (sidebarGroupsWithPersistence.has(group)) return
      sidebarGroupsWithPersistence.add(group)

      group.addEventListener('toggle', () => {
        if (syncingSidebarGroups) return

        const key = sidebarGroupKey(group)
        if (!key) return

        const state = readSidebarGroupState()
        state[key] = group.open
        writeSidebarGroupState(state)
        setMatchingSidebarGroups(key, group.open, group)
      })
    })
}

function sidebarViewports(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(SIDEBAR_VIEWPORT_SELECTOR))
}

function sidebarScrollKey(viewport: HTMLElement): string {
  const version =
    viewport.closest<HTMLElement>('[data-version-sidebar]')?.dataset.versionSidebar ?? 'default'
  const placement = viewport.closest('#mobile-sidebar') ? 'mobile' : 'desktop'
  return `${placement}:${version}`
}

function saveSidebarViewportScroll(viewport: HTMLElement): void {
  const state = readSidebarScrollState()
  state[sidebarScrollKey(viewport)] = viewport.scrollTop
  writeSidebarScrollState(state)
}

export function saveVisibleSidebarScrolls(): void {
  sidebarViewports().forEach((viewport) => {
    if (!isElementHidden(viewport)) saveSidebarViewportScroll(viewport)
  })
}

export function savePendingSidebarScroll(): void {
  const viewport = sidebarViewports().find((candidate) => !isElementHidden(candidate))
  if (!viewport) return

  writeStorage(sessionStorage, SIDEBAR_PENDING_SCROLL_KEY, String(viewport.scrollTop))
}

export function restoreSidebarScrolls(): void {
  const pendingScrollTop = readNumber(sessionStorage, SIDEBAR_PENDING_SCROLL_KEY)
  const state = readSidebarScrollState()
  let usedPendingScroll = false

  sidebarViewports().forEach((viewport) => {
    if (isElementHidden(viewport)) return

    const scrollTop = pendingScrollTop ?? state[sidebarScrollKey(viewport)]
    if (typeof scrollTop !== 'number') return

    viewport.scrollTop = Math.max(
      0,
      Math.min(scrollTop, viewport.scrollHeight - viewport.clientHeight),
    )
    if (pendingScrollTop !== null) usedPendingScroll = true
  })

  if (usedPendingScroll) removeStorage(sessionStorage, SIDEBAR_PENDING_SCROLL_KEY)
}

function setupPersistentSidebarScrolls(): void {
  restoreSidebarScrolls()

  sidebarViewports().forEach((viewport) => {
    if (sidebarViewportsWithPersistence.has(viewport)) return
    sidebarViewportsWithPersistence.add(viewport)

    viewport.addEventListener('scroll', () => saveSidebarViewportScroll(viewport), {
      passive: true,
    })
  })
}

function expandActiveLinkParents(link: HTMLElement): void {
  let node: HTMLElement | null = link.parentElement

  while (node) {
    if (node instanceof HTMLDetailsElement) node.open = true
    node = node.parentElement
  }
}

function forceGroupOpen(details: HTMLDetailsElement): void {
  if (details.open) return
  details.open = true
  const key = sidebarGroupKey(details)
  if (!key) return
  const state = readSidebarGroupState()
  if (state[key] === true) return
  state[key] = true
  writeSidebarGroupState(state)
}

/**
 * Group summaries contain a navigation Link plus a chevron toggle button.
 * Clicks anywhere on the summary (label, padding, badge) should navigate to
 * the group's overview; only the chevron toggles the accordion.
 *
 * We can't `stopPropagation` because that also blocks `<ClientRouter />`'s
 * link interception and triggers a full page reload. Instead, cancel the
 * click's defaults (toggle + native navigation) and drive the navigation
 * ourselves via `navigate()`.
 */
function handleOverviewLinkClick(event: MouseEvent): void {
  if (event.button !== 0) return
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
  if (event.defaultPrevented) return

  const target = event.target instanceof Element ? event.target : null
  if (!target) return

  // Chevron toggle → let native <summary> toggle behaviour run.
  if (target.closest('[data-sidebar-group-toggle]')) return

  const summary = target.closest<HTMLElement>('summary')
  const link =
    target.closest<HTMLAnchorElement>('a[data-sidebar-overview-link]') ??
    summary?.querySelector<HTMLAnchorElement>('a[data-sidebar-overview-link]') ??
    null
  if (!link) return

  const details = link.closest('details')
  if (!(details instanceof HTMLDetailsElement)) return

  event.preventDefault()
  forceGroupOpen(details)
  navigate(link.href)
}

function updateActiveSidebarLink(): void {
  const current = trimTrailingSlash(window.location.pathname)

  document.querySelectorAll<HTMLAnchorElement>('[data-sidebar-link]').forEach((link) => {
    const href = link.getAttribute('data-sidebar-link') ?? ''
    const isActive = trimTrailingSlash(href) === current

    if (isActive) {
      link.setAttribute('aria-current', 'page')
      expandActiveLinkParents(link)
    } else {
      link.removeAttribute('aria-current')
    }
  })
}

function setupSidebar(): void {
  withDatasetFlag('[data-sidebar-root]', 'sidebarSettling', () => {
    setupPersistentSidebarGroups()
    updateActiveSidebarLink()
    setupPersistentSidebarScrolls()
  })
}

export const mountDocsSidebar = mountClientModule({
  setup: setupSidebar,
  cleanup: saveVisibleSidebarScrolls,
  initOnce: () => {
    document.addEventListener('click', handleOverviewLinkClick)
  },
})
