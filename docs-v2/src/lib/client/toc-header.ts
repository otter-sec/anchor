import { findScrollAreaViewport } from './dom'
import { mountClientModule } from './lifecycle'
import {
  buildHeadingRegions,
  centerElementInScrollContainer,
  dataAttributeSelector,
  getContentHeadings,
  getVisibleHeadingIds,
  headingHtml,
  sameStringArray,
  setVerticalScrollMask,
  type HeadingRegion,
} from './toc'

type MobileTocState = {
  progressCircle: HTMLElement | null
  currentSectionText: HTMLElement | null
  detailsElement: HTMLDetailsElement | null
  listElement: HTMLElement | null
  scrollArea: HTMLElement | null
  headings: HTMLElement[]
  regions: HeadingRegion[]
  activeIds: string[]
}

const INITIAL_OVERVIEW_TEXT = 'Overview'
const PROGRESS_CIRCLE_RADIUS = 10
const PROGRESS_CIRCLE_CIRCUMFERENCE = 2 * Math.PI * PROGRESS_CIRCLE_RADIUS

const state: MobileTocState = {
  progressCircle: null,
  currentSectionText: null,
  detailsElement: null,
  listElement: null,
  scrollArea: null,
  headings: [],
  regions: [],
  activeIds: [],
}

let eventController: AbortController | null = null

function resetState(): void {
  const tocContainer = document.getElementById('mobile-toc-container')

  state.progressCircle = document.getElementById('mobile-toc-progress-circle')
  state.currentSectionText = document.getElementById('mobile-toc-current-section')
  state.detailsElement = tocContainer?.querySelector<HTMLDetailsElement>('details') ?? null
  state.listElement = document.getElementById('mobile-table-of-contents')
  state.scrollArea = findScrollAreaViewport(tocContainer)
  state.headings = []
  state.regions = []
  state.activeIds = []

  if (!state.progressCircle) return

  state.progressCircle.style.strokeDasharray = PROGRESS_CIRCLE_CIRCUMFERENCE.toString()
  state.progressCircle.style.strokeDashoffset = PROGRESS_CIRCLE_CIRCUMFERENCE.toString()
}

function buildRegions(): void {
  state.headings = getContentHeadings('scroll-pages')
  state.regions = buildHeadingRegions(state.headings)
}

function visibleHeadingIds(): string[] {
  return getVisibleHeadingIds(state.headings, state.regions)
}

function updateScrollMask(): void {
  if (!state.scrollArea) return

  setVerticalScrollMask(state.scrollArea, state.scrollArea, {
    top: 'mask-t-from-80%',
    bottom: 'mask-b-from-80%',
  })
}

function updateCurrentSectionText(headingIds: string[]): void {
  if (!state.currentSectionText) return

  if (headingIds.length === 0) {
    state.currentSectionText.textContent = INITIAL_OVERVIEW_TEXT
    return
  }

  const activeHtml = state.headings
    .filter((heading) => headingIds.includes(heading.id))
    .map((heading) => headingHtml(heading))
    .filter(Boolean)
    .join(', ')

  if (activeHtml) {
    state.currentSectionText.innerHTML = activeHtml
  } else {
    state.currentSectionText.textContent = INITIAL_OVERVIEW_TEXT
  }
}

function scrollToActiveHeading(activeHeadingId: string): void {
  if (!state.listElement || !state.scrollArea) return

  const activeItem = state.listElement.querySelector<HTMLElement>(
    dataAttributeSelector('data-heading-id', activeHeadingId),
  )
  if (activeItem) centerElementInScrollContainer(state.scrollArea, activeItem)
}

function updateMobileTocLinks(headingIds: string[]): void {
  if (!state.listElement || !state.currentSectionText) return

  state.listElement.querySelectorAll<HTMLElement>('.mobile-toc-item').forEach((item) => {
    const headingId = item.dataset.headingId
    item.classList.toggle('text-foreground', Boolean(headingId && headingIds.includes(headingId)))
  })

  if (headingIds.length > 0) scrollToActiveHeading(headingIds[0])

  updateCurrentSectionText(headingIds)
}

function updateProgressCircle(): void {
  if (!state.progressCircle) return

  const scrollableDistance = document.documentElement.scrollHeight - window.innerHeight
  const scrollProgress =
    scrollableDistance > 0 ? Math.min(Math.max(window.scrollY / scrollableDistance, 0), 1) : 0

  state.progressCircle.style.strokeDashoffset = (
    PROGRESS_CIRCLE_CIRCUMFERENCE *
    (1 - scrollProgress)
  ).toString()
}

function syncHeadingLabels(): void {
  if (!state.listElement) return

  state.listElement.querySelectorAll<HTMLElement>('.mobile-toc-item').forEach((link) => {
    const slug = link.dataset.headingId
    if (!slug) return

    const heading = document.getElementById(slug)
    const target = link.querySelector<HTMLElement>('[data-toc-text]')
    if (heading && target) target.innerHTML = headingHtml(heading)
  })
}

function handleScroll(): void {
  const newActiveIds = visibleHeadingIds()

  if (!sameStringArray(newActiveIds, state.activeIds)) {
    state.activeIds = newActiveIds
    updateMobileTocLinks(state.activeIds)
  }

  updateProgressCircle()
}

function handleResize(): void {
  buildRegions()

  const newActiveIds = visibleHeadingIds()
  if (!sameStringArray(newActiveIds, state.activeIds)) {
    state.activeIds = newActiveIds
    updateMobileTocLinks(state.activeIds)
  }

  updateProgressCircle()
}

function setupInteractions(signal: AbortSignal): void {
  state.listElement?.querySelectorAll('.mobile-toc-item').forEach((item) => {
    item.addEventListener(
      'click',
      () => {
        if (state.detailsElement) state.detailsElement.open = false
      },
      { signal },
    )
  })

  state.scrollArea?.addEventListener('scroll', updateScrollMask, { passive: true, signal })

  state.detailsElement?.addEventListener(
    'toggle',
    () => {
      if (state.detailsElement?.open) window.setTimeout(updateScrollMask, 100)
    },
    { signal },
  )
}

function cleanupMobileTocHeader(): void {
  eventController?.abort()
  eventController = null

  state.activeIds = []
  state.headings = []
  state.regions = []
}

function initMobileTocHeader(): void {
  cleanupMobileTocHeader()
  resetState()

  if (!state.currentSectionText) return

  eventController = new AbortController()
  const { signal } = eventController

  syncHeadingLabels()
  buildRegions()

  if (state.headings.length === 0) {
    state.currentSectionText.textContent = INITIAL_OVERVIEW_TEXT
    window.addEventListener('scroll', updateProgressCircle, { passive: true, signal })
    updateProgressCircle()
    return
  }

  state.activeIds = visibleHeadingIds()
  updateMobileTocLinks(state.activeIds)
  updateProgressCircle()
  setupInteractions(signal)

  window.addEventListener('scroll', handleScroll, { passive: true, signal })
  window.addEventListener('resize', handleResize, { passive: true, signal })
}

export const mountMobileTocHeader = mountClientModule({
  setup: initMobileTocHeader,
  cleanup: cleanupMobileTocHeader,
})
