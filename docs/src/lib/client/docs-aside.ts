import { findScrollAreaViewport } from './dom'
import { mountClientModule } from './lifecycle'

let cleanupScrollFades: (() => void) | null = null

function setupScrollFades(): void {
  cleanupScrollFades?.()
  cleanupScrollFades = null

  const nav = document.querySelector<HTMLElement>('[data-sidebar-nav]')
  const viewport = findScrollAreaViewport(nav)
  if (!nav || !viewport) return

  function update(): void {
    const atTop = viewport!.scrollTop <= 1
    const atBottom = viewport!.scrollTop + viewport!.clientHeight >= viewport!.scrollHeight - 1

    nav!.style.maskImage =
      atTop && atBottom
        ? 'none'
        : atTop
          ? 'linear-gradient(to bottom, black calc(100% - 2rem), transparent)'
          : atBottom
            ? 'linear-gradient(to bottom, transparent, black 2rem)'
            : 'linear-gradient(to bottom, transparent, black 2rem, black calc(100% - 2rem), transparent)'
  }

  update()
  viewport.addEventListener('scroll', update, { passive: true })

  const observer = new ResizeObserver(update)
  observer.observe(viewport)

  cleanupScrollFades = () => {
    viewport.removeEventListener('scroll', update)
    observer.disconnect()
    nav.style.maskImage = ''
  }
}

function updateEditLink(): void {
  const link = document.getElementById('docs-edit-link')
  if (!link) return

  const meta = document.querySelector<HTMLMetaElement>('meta[name="docs-edit-url"]')
  const url = meta?.content ?? ''

  link.setAttribute('href', url || '#')
  link.toggleAttribute('hidden', !url)
}

export const mountDocsAside = mountClientModule({
  setup: () => {
    setupScrollFades()
    updateEditLink()
  },
  cleanup: () => cleanupScrollFades?.(),
})
