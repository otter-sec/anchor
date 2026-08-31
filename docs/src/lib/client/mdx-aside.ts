import { mountClientModule } from './lifecycle'
import { setupTabs } from './tabs'

const ASIDE_LAYOUT_SELECTOR = '[data-mdx-aside-layout]'

function directAsides(layout: HTMLElement): HTMLElement[] {
  return Array.from(layout.children).filter(
    (child): child is HTMLElement =>
      child instanceof HTMLElement &&
      child.matches('[data-mdx-aside]') &&
      !child.hasAttribute('data-mdx-aside-clone'),
  )
}

function removeIds(element: HTMLElement): void {
  element.removeAttribute('id')
  element.querySelectorAll<HTMLElement>('[id]').forEach((child) => child.removeAttribute('id'))
}

function resetClonedClientState(element: HTMLElement): void {
  element
    .querySelectorAll<HTMLElement>('[data-tabs-init]')
    .forEach((child) => delete child.dataset.tabsInit)
}

function cloneAside(aside: HTMLElement): HTMLElement {
  const clone = aside.cloneNode(true) as HTMLElement
  clone.removeAttribute('data-mdx-aside-source')
  clone.setAttribute('data-mdx-aside-clone', '')
  removeIds(clone)
  resetClonedClientState(clone)
  return clone
}

function asideContentBottom(asides: HTMLElement[]): number {
  return Math.max(...asides.map((aside) => aside.offsetTop + aside.offsetHeight))
}

function setupAsideLayout(layout: HTMLElement): void {
  const asides = directAsides(layout)
  if (asides.length === 0) return
  layout.style.setProperty('--mdx-aside-min-height', `${asideContentBottom(asides)}px`)

  const rail = document.createElement('div')
  rail.setAttribute('data-mdx-aside-rail', '')
  rail.setAttribute('data-pagefind-ignore', '')
  rail.style.setProperty('--mdx-aside-offset', `${asides[0].offsetTop}px`)

  asides.forEach((aside) => {
    rail.append(cloneAside(aside))
    aside.setAttribute('data-mdx-aside-source', '')
  })

  layout.append(rail)
  setupTabs()
}

function cleanupMdxAsides(): void {
  document.querySelectorAll('[data-mdx-aside-rail]').forEach((rail) => rail.remove())
  document
    .querySelectorAll<HTMLElement>(ASIDE_LAYOUT_SELECTOR)
    .forEach((layout) => layout.style.removeProperty('--mdx-aside-min-height'))
  document
    .querySelectorAll<HTMLElement>('[data-mdx-aside-source]')
    .forEach((aside) => aside.removeAttribute('data-mdx-aside-source'))
}

function initMdxAsides(): void {
  cleanupMdxAsides()
  document.querySelectorAll<HTMLElement>(ASIDE_LAYOUT_SELECTOR).forEach(setupAsideLayout)
}

export const mountMdxAsides = mountClientModule({
  setup: initMdxAsides,
  cleanup: cleanupMdxAsides,
})
