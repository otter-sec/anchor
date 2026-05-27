export type HeadingRegion = {
  id: string
  start: number
  end: number
}

const CONTENT_HEADING_SELECTOR = [
  '.prose h2:not([data-scroll-page-title])',
  '.prose h3',
  '.prose h4',
  '.prose h5',
  '.prose h6',
].join(', ')
const SCROLL_GROUP_HEADING_SELECTOR = '.prose [data-scroll-page-title]'
const DEFAULT_HEADER_OFFSET = 120
const DEFAULT_SCROLL_THRESHOLD = 5

export function normalizeGroupedHeadingIds(): void {
  document.querySelectorAll<HTMLElement>('[data-scroll-page-section]').forEach((section) => {
    const prefix = section.dataset.scrollPagePrefix
    if (!prefix || section.dataset.scrollPageHeadingsNormalized === 'true') return

    section.querySelectorAll<HTMLElement>('h2, h3, h4, h5, h6').forEach((heading) => {
      if (!heading.id || heading.hasAttribute('data-scroll-page-title')) return

      const oldId = heading.id
      const newId = `${prefix}-${oldId}`
      heading.id = newId

      section.querySelectorAll<HTMLAnchorElement>('a[href]').forEach((link) => {
        if (link.getAttribute('href') === `#${oldId}`) link.setAttribute('href', `#${newId}`)
      })
    })

    section.dataset.scrollPageHeadingsNormalized = 'true'
  })
}

export function getContentHeadings(mode: 'content' | 'scroll-pages' = 'content'): HTMLElement[] {
  normalizeGroupedHeadingIds()

  const selector =
    mode === 'scroll-pages' && document.querySelector('[data-scroll-page-section]')
      ? SCROLL_GROUP_HEADING_SELECTOR
      : CONTENT_HEADING_SELECTOR

  return Array.from(document.querySelectorAll<HTMLElement>(selector)).filter(
    (heading) => Boolean(heading.id) && heading.offsetParent !== null,
  )
}

export function buildHeadingRegions(headings: HTMLElement[]): HeadingRegion[] {
  return headings.map((heading, index) => {
    const nextHeading = headings[index + 1]
    return {
      id: heading.id,
      start: heading.offsetTop,
      end: nextHeading ? nextHeading.offsetTop : document.body.scrollHeight,
    }
  })
}

function isInViewport(top: number, bottom: number, viewportTop: number, viewportBottom: number) {
  return (
    (top >= viewportTop && top <= viewportBottom) ||
    (bottom >= viewportTop && bottom <= viewportBottom) ||
    (top <= viewportTop && bottom >= viewportBottom)
  )
}

export function getVisibleHeadingIds(
  headings: HTMLElement[],
  regions: HeadingRegion[],
  offset = DEFAULT_HEADER_OFFSET,
): string[] {
  if (headings.length === 0) return []

  const viewportTop = window.scrollY + offset
  const viewportBottom = window.scrollY + window.innerHeight
  const visibleIds = new Set<string>()

  headings.forEach((heading) => {
    const headingBottom = heading.offsetTop + heading.offsetHeight
    if (isInViewport(heading.offsetTop, headingBottom, viewportTop, viewportBottom)) {
      visibleIds.add(heading.id)
    }
  })

  regions.forEach((region) => {
    if (region.start > viewportBottom || region.end < viewportTop) return

    const heading = document.getElementById(region.id)
    if (!heading) return

    const headingBottom = heading.offsetTop + heading.offsetHeight
    if (
      region.end > headingBottom &&
      (headingBottom < viewportBottom || viewportTop < region.end)
    ) {
      visibleIds.add(region.id)
    }
  })

  return Array.from(visibleIds)
}

export function sameStringArray(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index])
}

export function centerElementInScrollContainer(container: HTMLElement, element: Element): void {
  const { top: containerTop, height: containerHeight } = container.getBoundingClientRect()
  const { top: elementTop, height: elementHeight } = element.getBoundingClientRect()

  const currentElementTop = elementTop - containerTop + container.scrollTop
  const maxScroll = container.scrollHeight - container.clientHeight
  const targetScroll = Math.max(
    0,
    Math.min(currentElementTop - (containerHeight - elementHeight) / 2, maxScroll),
  )

  if (Math.abs(targetScroll - container.scrollTop) > DEFAULT_SCROLL_THRESHOLD) {
    container.scrollTop = targetScroll
  }
}

export function setVerticalScrollMask(
  scroller: HTMLElement,
  target: HTMLElement,
  classes: { top: string; bottom: string },
  threshold = DEFAULT_SCROLL_THRESHOLD,
): void {
  const { scrollTop, scrollHeight, clientHeight } = scroller
  const isAtTop = scrollTop <= threshold
  const isAtBottom = scrollTop >= scrollHeight - clientHeight - threshold

  target.classList.toggle(classes.top, !isAtTop)
  target.classList.toggle(classes.bottom, !isAtBottom)
}

export function headingHtml(
  heading: HTMLElement,
  options: { longPillThreshold?: number } = {},
): string {
  const { longPillThreshold } = options
  const clone = heading.cloneNode(true) as HTMLElement
  clone.querySelectorAll('.heading-anchor').forEach((el) => el.remove())
  clone.querySelectorAll<HTMLElement>('.shiki').forEach((el) => {
    el.removeAttribute('style')
  })

  if (longPillThreshold !== undefined) {
    clone.querySelectorAll<HTMLElement>('.shiki, code').forEach((el) => {
      if ((el.textContent ?? '').length > longPillThreshold) {
        el.dataset.longPill = 'true'
      }
    })
  }

  return clone.innerHTML.trim()
}

export function dataAttributeSelector(attribute: string, value: string): string {
  return `[${attribute}="${CSS.escape(value)}"]`
}
