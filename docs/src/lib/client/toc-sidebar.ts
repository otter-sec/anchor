import { findScrollAreaViewport, withDatasetFlag } from './dom'
import { mountClientModule } from './lifecycle'
import {
  buildHeadingRegions,
  centerElementInScrollContainer,
  getContentHeadings,
  getVisibleHeadingIds,
  headingHtml,
  sameStringArray,
  setVerticalScrollMask,
  type HeadingRegion,
} from './toc'

type TocSidebarState = {
  roots: TocSidebarRootState[]
  links: HTMLElement[]
  activeIds: string[]
  headings: HTMLElement[]
  regions: HeadingRegion[]
}

type TocSidebarRootState = {
  root: HTMLElement
  links: HTMLElement[]
  scrollArea: HTMLElement | null
  tocScrollArea: HTMLElement | null
}

const state: TocSidebarState = {
  roots: [],
  links: [],
  activeIds: [],
  headings: [],
  regions: [],
}

let eventController: AbortController | null = null
let visibilityObserver: MutationObserver | null = null
let titleObserver: IntersectionObserver | null = null

function resetState(): void {
  state.roots = Array.from(document.querySelectorAll<HTMLElement>('[data-toc-sidebar-root]')).map(
    (root) => {
      const tocScrollArea = root.querySelector<HTMLElement>('[data-toc-scroll-area]')
      const links = Array.from(root.querySelectorAll<HTMLElement>('[data-heading-link]'))

      return {
        root,
        links,
        tocScrollArea,
        scrollArea: findScrollAreaViewport(tocScrollArea ?? root),
      }
    },
  )

  state.links = state.roots.flatMap((root) => root.links)
  state.activeIds = []
  state.headings = []
  state.regions = []
}

function buildRegions(): void {
  state.headings = getContentHeadings()
  state.regions = buildHeadingRegions(state.headings)
}

function visibleHeadingIds(): string[] {
  return getVisibleHeadingIds(state.headings, state.regions)
}

function updateScrollMask(): void {
  state.roots.forEach((root) => {
    if (!root.scrollArea || !root.tocScrollArea) return

    setVerticalScrollMask(root.scrollArea, root.tocScrollArea, {
      top: 'mask-t-from-90%',
      bottom: 'mask-b-from-90%',
    })
  })
}

function linkForHeading(headingId: string): HTMLElement | null {
  return state.links.find((link) => link.dataset.headingLink === headingId) ?? null
}

function scrollToActiveHeading(headingIds: string[]): void {
  if (headingIds.length === 0) return

  state.roots.forEach((root) => {
    if (!root.scrollArea) return

    const activeLink = headingIds
      .map((id) => root.links.find((link) => link.dataset.headingLink === id))
      .find((link) => link !== undefined)

    if (activeLink) centerElementInScrollContainer(root.scrollArea, activeLink)
  })
}

function updateActiveLinks(headingIds: string[]): void {
  state.links.forEach((link) => link.classList.remove('text-foreground'))

  headingIds.forEach((id) => {
    linkForHeading(id)?.classList.add('text-foreground')
  })

  scrollToActiveHeading(headingIds)
}

function syncItemVisibility(visibleHeadings: HTMLElement[]): void {
  const visibleIds = new Set(visibleHeadings.map((heading) => heading.id))

  document
    .querySelectorAll<HTMLElement>('[data-toc-sidebar-root] [data-toc-item]')
    .forEach((item) => {
      const slug = item.dataset.tocItem
      item.hidden = !slug || !visibleIds.has(slug)
    })
}

function syncHeadingLabels(): void {
  state.links.forEach((link) => {
    const slug = link.dataset.headingLink
    if (!slug) return

    const heading = document.getElementById(slug)
    const target = link.querySelector<HTMLElement>('[data-toc-text]')
    if (!heading || !target) return

    target.innerHTML = headingHtml(heading, {
      longPillThreshold: 24,
    })
  })
}

function cleanupTitleVisibility(): void {
  titleObserver?.disconnect()
  titleObserver = null
}

function titleTargetForWrapper(wrapper: HTMLElement): HTMLElement | null {
  const targetId = wrapper.dataset.tocTitleTarget ?? 'post-title'
  return document.getElementById(targetId)
}

function updateTitleWrapperVisibility(wrapper: HTMLElement, target: HTMLElement): void {
  wrapper.dataset.open = target.getBoundingClientRect().bottom <= 0 ? 'true' : 'false'
}

function setupTitleVisibility(): void {
  const wrappers = Array.from(
    document.querySelectorAll<HTMLElement>('[data-toc-sidebar-title-wrapper]'),
  )
  if (wrappers.length === 0) return

  cleanupTitleVisibility()

  const targetToWrappers = new Map<HTMLElement, HTMLElement[]>()

  wrappers.forEach((wrapper) => {
    const target = titleTargetForWrapper(wrapper)
    if (!target) return

    updateTitleWrapperVisibility(wrapper, target)
    const existing = targetToWrappers.get(target) ?? []
    existing.push(wrapper)
    targetToWrappers.set(target, existing)
  })

  if (targetToWrappers.size === 0) return

  titleObserver = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        const target = entry.target
        if (!(target instanceof HTMLElement)) return

        const open =
          !entry.isIntersecting && entry.boundingClientRect.bottom <= 0 ? 'true' : 'false'

        targetToWrappers.get(target)?.forEach((wrapper) => {
          wrapper.dataset.open = open
        })
      })
    },
    { rootMargin: '0px', threshold: 0 },
  )

  targetToWrappers.forEach((_wrappers, target) => {
    titleObserver?.observe(target)
  })
}

function handleContentScroll(): void {
  const newActiveIds = visibleHeadingIds()

  if (!sameStringArray(newActiveIds, state.activeIds)) {
    state.activeIds = newActiveIds
    updateActiveLinks(state.activeIds)
  }
}

function handleResize(): void {
  buildRegions()
  const newActiveIds = visibleHeadingIds()

  if (!sameStringArray(newActiveIds, state.activeIds)) {
    state.activeIds = newActiveIds
    updateActiveLinks(state.activeIds)
  }

  syncItemVisibility(state.headings)
  updateScrollMask()
}

function observeVisibilityChanges(): void {
  const root = document.querySelector('.prose')
  if (!root) return

  visibilityObserver = new MutationObserver((mutations) => {
    const relevant = mutations.some(
      (mutation) =>
        mutation.type === 'attributes' &&
        (mutation.attributeName === 'hidden' || mutation.attributeName === 'open'),
    )

    if (relevant) handleResize()
  })

  visibilityObserver.observe(root, {
    subtree: true,
    attributes: true,
    attributeFilter: ['hidden', 'open'],
  })
}

function cleanupTocSidebar(): void {
  eventController?.abort()
  eventController = null

  visibilityObserver?.disconnect()
  visibilityObserver = null
  cleanupTitleVisibility()

  Object.assign(state, {
    links: [],
    activeIds: [],
    headings: [],
    regions: [],
    roots: [],
  })
}

function initTocSidebar(): void {
  cleanupTocSidebar()
  resetState()

  let hasHeadings = false

  withDatasetFlag('[data-toc-sidebar-root]', 'tocSettling', () => {
    buildRegions()
    syncHeadingLabels()
    setupTitleVisibility()
    hasHeadings = state.headings.length > 0

    if (!hasHeadings) {
      updateActiveLinks([])
      syncItemVisibility([])
      return
    }

    handleContentScroll()
    syncItemVisibility(state.headings)
  })

  if (!hasHeadings) return

  eventController = new AbortController()
  const { signal } = eventController

  updateScrollMask()

  window.addEventListener('scroll', handleContentScroll, { passive: true, signal })
  window.addEventListener('resize', handleResize, { passive: true, signal })
  state.roots.forEach((root) => {
    root.scrollArea?.addEventListener('scroll', updateScrollMask, { passive: true, signal })
  })
  observeVisibilityChanges()
}

export const mountTocSidebar = mountClientModule({
  setup: initTocSidebar,
  cleanup: cleanupTocSidebar,
})
