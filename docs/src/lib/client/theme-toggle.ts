import { mountClientModule } from './lifecycle'
import { readStorage, writeStorage } from './storage'

let controller: AbortController | null = null

function toggleTheme(): void {
  const element = document.documentElement
  const currentTheme = element.getAttribute('data-theme')
  const newTheme = currentTheme === 'dark' ? 'light' : 'dark'

  element.classList.add('[&_*]:transition-none')
  element.setAttribute('data-theme', newTheme)
  window.getComputedStyle(element).getPropertyValue('opacity')

  requestAnimationFrame(() => {
    element.classList.remove('[&_*]:transition-none')
  })

  writeStorage(localStorage, 'theme', newTheme)
}

function initThemeToggle(): void {
  controller?.abort()
  controller = new AbortController()
  const { signal } = controller

  document
    .querySelectorAll<HTMLButtonElement>('[data-theme-toggle]')
    .forEach((button) => button.addEventListener('click', toggleTheme, { signal }))
}

function beforeSwap(event: Event): void {
  controller?.abort()

  const storedTheme = readStorage(localStorage, 'theme') || 'light'
  ;(event as Event & { newDocument: Document }).newDocument.documentElement.setAttribute(
    'data-theme',
    storedTheme,
  )
}

export const mountThemeToggle = mountClientModule({
  setup: initThemeToggle,
  // `beforeSwap` both aborts old click listeners and propagates the stored
  // theme onto the incoming document before paint to avoid a flash.
  cleanup: beforeSwap,
})
