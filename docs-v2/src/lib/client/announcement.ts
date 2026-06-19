import { mountClientModule } from './lifecycle'
import { readStorage, writeStorage } from './storage'

const ANNOUNCEMENT_SELECTOR = '[data-announcement-id]'
const DISMISSED_ATTRIBUTE = 'data-docs-announcement-dismissed'

function storageKey(bar: HTMLElement): string | null {
  const id = bar.dataset.announcementId
  return id ? `announcement-dismissed:${id}` : null
}

function isDismissed(bar: HTMLElement): boolean {
  const key = storageKey(bar)
  return key ? readStorage(localStorage, key) === '1' : false
}

function dismiss(bar: HTMLElement): void {
  const key = storageKey(bar)
  if (key) writeStorage(localStorage, key, '1')
}

export function updateAnnouncementOffset(): void {
  const bar = document.querySelector<HTMLElement>(`${ANNOUNCEMENT_SELECTOR}:not([hidden])`)
  const offset = bar
    ? Math.max(0, Math.min(bar.offsetHeight, bar.getBoundingClientRect().bottom))
    : 0

  document.documentElement.style.setProperty('--docs-announcement-offset', `${offset}px`)
}

function applyDismissedState(): void {
  const bars = Array.from(document.querySelectorAll<HTMLElement>(ANNOUNCEMENT_SELECTOR))
  let hasDismissedAnnouncement = false

  bars.forEach((bar) => {
    if (!isDismissed(bar)) return

    hasDismissedAnnouncement = true
    bar.remove()
  })

  if (hasDismissedAnnouncement) {
    document.documentElement.setAttribute(DISMISSED_ATTRIBUTE, '')
  } else {
    document.documentElement.removeAttribute(DISMISSED_ATTRIBUTE)
  }

  updateAnnouncementOffset()
}

function handleDismissClick(event: MouseEvent): void {
  const target = event.target instanceof HTMLElement ? event.target : null
  const button = target?.closest<HTMLElement>('[data-announcement-dismiss]')
  const bar = button?.closest<HTMLElement>(ANNOUNCEMENT_SELECTOR)
  if (!bar) return

  dismiss(bar)
  document.documentElement.setAttribute(DISMISSED_ATTRIBUTE, '')
  bar.remove()
  updateAnnouncementOffset()
}

// Click/scroll/resize are delegated to `document`/`window` once and survive
// CSN navigations, so no per-page cleanup is needed.
export const mountAnnouncementBar = mountClientModule({
  setup: applyDismissedState,
  initOnce: () => {
    document.addEventListener('click', handleDismissClick)
    window.addEventListener('scroll', updateAnnouncementOffset, { passive: true })
    window.addEventListener('resize', updateAnnouncementOffset, { passive: true })
  },
})
