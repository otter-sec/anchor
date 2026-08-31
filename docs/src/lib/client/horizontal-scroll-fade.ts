type HorizontalScrollFadeState = {
  cleanups: WeakMap<HTMLElement, () => void>
  activeCleanups: Set<() => void>
}

type HorizontalScrollFadeOptions = {
  frameSelector?: string
  itemSelector?: string
}

const DEFAULT_FRAME_SELECTOR = '[data-bento-scroll-frame]'

const state: HorizontalScrollFadeState = {
  cleanups: new WeakMap(),
  activeCleanups: new Set(),
}

function px(value: string): number {
  const number = Number.parseFloat(value)
  return Number.isFinite(number) ? number : 0
}

function childElements(element: HTMLElement): HTMLElement[] {
  return Array.from(element.children).filter(
    (child): child is HTMLElement => child instanceof HTMLElement,
  )
}

function scrollItems(section: HTMLElement, selector?: string): HTMLElement[] {
  if (!selector) return childElements(section)
  return Array.from(section.querySelectorAll<HTMLElement>(selector))
}

function snappedEdges(
  section: HTMLElement,
  options: HorizontalScrollFadeOptions,
): { left: boolean; right: boolean } {
  const styles = window.getComputedStyle(section)
  const rect = section.getBoundingClientRect()
  const leftEdge = rect.left + px(styles.paddingLeft) + px(styles.scrollPaddingLeft)
  const rightEdge = rect.right - px(styles.paddingRight) - px(styles.scrollPaddingRight)
  const snapThreshold = 3
  const items = scrollItems(section, options.itemSelector)

  return {
    left: items.some(
      (item) => Math.abs(item.getBoundingClientRect().left - leftEdge) <= snapThreshold,
    ),
    right: items.some(
      (item) => Math.abs(item.getBoundingClientRect().right - rightEdge) <= snapThreshold,
    ),
  }
}

export function bindHorizontalScrollFade(
  section: HTMLElement,
  options: HorizontalScrollFadeOptions = {},
): () => void {
  state.cleanups.get(section)?.()

  const frame = section.closest<HTMLElement>(options.frameSelector ?? DEFAULT_FRAME_SELECTOR)
  if (!frame) return () => {}

  let animationFrame = 0

  const update = () => {
    animationFrame = 0

    const { scrollLeft, scrollWidth, clientWidth } = section
    const threshold = 2
    const overflow = scrollWidth - clientWidth
    const isAtStart = scrollLeft <= threshold
    const isAtEnd = overflow <= threshold || scrollLeft >= overflow - threshold
    const snapped = snappedEdges(section, options)

    frame.dataset.leftFade = !isAtStart && !snapped.left ? 'true' : 'false'
    frame.dataset.rightFade = !isAtEnd && !snapped.right ? 'true' : 'false'
  }

  const scheduleUpdate = () => {
    if (animationFrame) return
    animationFrame = window.requestAnimationFrame(update)
  }

  const cleanup = () => {
    if (animationFrame) window.cancelAnimationFrame(animationFrame)
    section.removeEventListener('scroll', scheduleUpdate)
    window.removeEventListener('resize', scheduleUpdate)
    frame.dataset.leftFade = 'false'
    frame.dataset.rightFade = 'false'
    state.cleanups.delete(section)
    state.activeCleanups.delete(cleanup)
  }

  update()
  section.addEventListener('scroll', scheduleUpdate, { passive: true })
  window.addEventListener('resize', scheduleUpdate)

  state.cleanups.set(section, cleanup)
  state.activeCleanups.add(cleanup)

  return cleanup
}

function setupHorizontalScrollFade(section: HTMLElement): void {
  bindHorizontalScrollFade(section)
}

export function cleanupHorizontalScrollFades(): void {
  state.activeCleanups.forEach((cleanup) => cleanup())
  state.activeCleanups.clear()
}

export function setupHorizontalScrollFades(): void {
  document.querySelectorAll<HTMLElement>('[data-bento-scroll]').forEach(setupHorizontalScrollFade)
}

let lifecycleReady = false

export function mountHorizontalScrollFades(): void {
  setupHorizontalScrollFades()

  if (lifecycleReady) return
  lifecycleReady = true

  document.addEventListener('astro:before-swap', cleanupHorizontalScrollFades)
  document.addEventListener('astro:after-swap', setupHorizontalScrollFades)
}
