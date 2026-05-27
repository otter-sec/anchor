import type { DocsVersion } from '@/lib/docs-versions'
import { trimTrailingSlash } from './dom'
import { savePendingSidebarScroll } from './sidebar'
import { readStorage, writeStorage } from './storage'

const STORAGE_KEY = 'anchor-docs-version'
const BASE_PATH = import.meta.env.BASE_URL.replace(/\/$/, '')

let controller: AbortController | null = null
let lifecycleReady = false

function isDocsHomePath(path: string): boolean {
  return trimTrailingSlash(path) === BASE_PATH
}

function versionFromPath(path: string): DocsVersion | null {
  if (path === `${BASE_PATH}/v1` || path.startsWith(`${BASE_PATH}/v1/`)) return 'v1'
  if (path === `${BASE_PATH}/v2` || path.startsWith(`${BASE_PATH}/v2/`)) return 'v2'
  return null
}

function versionFromUrl(): DocsVersion | null {
  if (isDocsHomePath(window.location.pathname)) return null

  const pathVersion = versionFromPath(window.location.pathname)
  if (pathVersion) return pathVersion

  const params = new URLSearchParams(window.location.search)
  const queryVersion = params.get('version')
  if (queryVersion === 'v1' || queryVersion === 'v2') return queryVersion

  const stored = readStorage(localStorage, STORAGE_KEY)
  return stored === 'v1' || stored === 'v2' ? stored : 'v2'
}

function setVersionSidebarFocus(version: DocsVersion | null): void {
  document.querySelectorAll<HTMLElement>('[data-version-sidebar]').forEach((sidebar) => {
    sidebar.hidden = version ? sidebar.dataset.versionSidebar !== version : false
  })
}

function setVersionOptionFocus(version: DocsVersion | null): void {
  document.querySelectorAll<HTMLAnchorElement>('[data-version-option]').forEach((option) => {
    const active = version !== null && option.dataset.versionOption === version
    if (active) option.dataset.active = 'true'
    else option.removeAttribute('data-active')

    const optionUrl = new URL(option.href)
    const samePath =
      trimTrailingSlash(optionUrl.pathname) === trimTrailingSlash(window.location.pathname)
    if (active && samePath) option.setAttribute('aria-current', 'page')
    else option.removeAttribute('aria-current')
  })
}

function applyVersionFocus(): void {
  const version = versionFromUrl()
  if (version) writeStorage(localStorage, STORAGE_KEY, version)

  setVersionSidebarFocus(version)
  setVersionOptionFocus(version)
  window.dispatchEvent(new CustomEvent('anchor-docs:version-focus'))
}

function setupVersionSwitcher(): void {
  controller?.abort()
  controller = new AbortController()
  const { signal } = controller

  applyVersionFocus()

  document.querySelectorAll<HTMLAnchorElement>('[data-version-option]').forEach((option) => {
    option.addEventListener(
      'click',
      () => {
        savePendingSidebarScroll()

        const version = option.dataset.versionOption
        if (version === 'v1' || version === 'v2') {
          writeStorage(localStorage, STORAGE_KEY, version)
        }
      },
      { signal },
    )
  })
}

export function mountVersionSwitcher(): void {
  setupVersionSwitcher()

  if (lifecycleReady) return
  lifecycleReady = true

  document.addEventListener('astro:before-swap', () => controller?.abort())
  document.addEventListener('astro:after-swap', setupVersionSwitcher)
}
