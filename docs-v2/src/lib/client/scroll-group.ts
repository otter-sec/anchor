import { findScrollAreaViewport, isElementHidden, trimTrailingSlash } from './dom'
import { mountClientModule } from './lifecycle'
import { centerElementInScrollContainer, normalizeGroupedHeadingIds } from './toc'

type ScrollGroupSection = {
  element: HTMLElement
  target: HTMLElement
  href: string
  path: string
}

const HEADER_OFFSET = 120
const BOTTOM_THRESHOLD = 2

let controller: AbortController | null = null
let sections: ScrollGroupSection[] = []
let activePath: string | null = null

function pathFromHref(href: string): string | null {
  try {
    return trimTrailingSlash(new URL(href, window.location.origin).pathname)
  } catch {
    return null
  }
}

function readSections(): ScrollGroupSection[] {
  return Array.from(document.querySelectorAll<HTMLElement>('[data-scroll-page-section]')).flatMap(
    (element) => {
      const href = element.dataset.scrollPageHref
      const targetId = element.dataset.scrollPageTarget
      const target = targetId ? document.getElementById(targetId) : element
      const path = href ? pathFromHref(href) : null

      if (!href || !target || !path) return []
      return [{ element, target, href, path }]
    },
  )
}

function sectionForPath(path: string): ScrollGroupSection | null {
  const normalized = trimTrailingSlash(path)
  return sections.find((section) => section.path === normalized) ?? null
}

function isAtDocumentBottom(): boolean {
  const scroller = document.scrollingElement ?? document.documentElement
  const maxScrollY = scroller.scrollHeight - window.innerHeight

  if (maxScrollY <= 0) return false
  return window.scrollY >= maxScrollY - BOTTOM_THRESHOLD
}

function activeSection(): ScrollGroupSection | null {
  const lastSection = sections[sections.length - 1] ?? null
  if (lastSection && isAtDocumentBottom()) return lastSection

  const threshold = window.scrollY + HEADER_OFFSET
  let active = sections[0] ?? null

  for (const section of sections) {
    if (section.element.offsetTop <= threshold) active = section
  }

  return active
}

function sidebarLinks(): HTMLAnchorElement[] {
  return Array.from(document.querySelectorAll<HTMLAnchorElement>('[data-sidebar-link]'))
}

function expandActiveLinkParents(link: HTMLElement): void {
  let node: HTMLElement | null = link.parentElement

  while (node) {
    if (node instanceof HTMLDetailsElement) node.open = true
    node = node.parentElement
  }
}

function centerVisibleSidebarLink(link: HTMLElement): void {
  const nav = link.closest<HTMLElement>('nav[aria-label="Documentation"]')
  const viewport = findScrollAreaViewport(nav)
  if (!viewport || isElementHidden(viewport)) return

  centerElementInScrollContainer(viewport, link)
}

function updateSidebar(section: ScrollGroupSection): void {
  sidebarLinks().forEach((link) => {
    const href = link.getAttribute('data-sidebar-link') ?? link.href
    const path = pathFromHref(href)
    const isActive = path === section.path

    if (isActive) {
      link.setAttribute('aria-current', 'page')
      expandActiveLinkParents(link)
      centerVisibleSidebarLink(link)
    } else {
      link.removeAttribute('aria-current')
    }
  })
}

function updateUrl(section: ScrollGroupSection): void {
  if (trimTrailingSlash(window.location.pathname) === section.path) return
  window.history.replaceState(window.history.state, '', section.href)
}

function activateSection(section: ScrollGroupSection, options: { updateUrl: boolean }): void {
  if (activePath !== section.path) {
    activePath = section.path
    updateSidebar(section)
  }

  if (options.updateUrl) updateUrl(section)
}

function scrollToSection(section: ScrollGroupSection, behavior: ScrollBehavior): void {
  if (section === sections[0]) {
    window.scrollTo({ top: 0, behavior })
    return
  }
  section.target.scrollIntoView({ behavior, block: 'start' })
}

function handleScroll(): void {
  const section = activeSection()
  if (section) activateSection(section, { updateUrl: true })
}

function shouldHandleClick(event: MouseEvent, link: HTMLAnchorElement): boolean {
  if (event.defaultPrevented || event.button !== 0) return false
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return false
  if (link.download) return false

  const target = link.getAttribute('target')
  return !target || target === '_self'
}

function handleDocumentClick(event: MouseEvent): void {
  const target = event.target
  if (!(target instanceof Element)) return

  const link = target.closest<HTMLAnchorElement>('a[href]')
  if (!link || !shouldHandleClick(event, link)) return

  const url = new URL(link.href)
  if (url.origin !== window.location.origin || url.hash) return

  const section = sectionForPath(url.pathname)
  if (!section) return

  event.preventDefault()
  scrollToSection(section, 'smooth')
  activateSection(section, { updateUrl: true })
}

function scheduleInitialScroll(): void {
  if (window.location.hash) return

  const section = sectionForPath(window.location.pathname)
  if (!section) return

  scrollToSection(section, 'auto')
  activateSection(section, { updateUrl: false })
}

function cleanupScrollGroup(): void {
  controller?.abort()
  controller = null
  sections = []
  activePath = null
}

function initScrollGroup(): void {
  cleanupScrollGroup()
  sections = readSections()
  if (sections.length === 0) return

  normalizeGroupedHeadingIds()

  controller = new AbortController()
  const { signal } = controller

  document.addEventListener('click', handleDocumentClick, { signal })
  window.addEventListener('scroll', handleScroll, { passive: true, signal })
  window.addEventListener('resize', handleScroll, { passive: true, signal })

  activateSection(activeSection() ?? sections[0], { updateUrl: false })
  scheduleInitialScroll()
}

export const mountScrollGroup = mountClientModule({
  setup: initScrollGroup,
  cleanup: cleanupScrollGroup,
})
