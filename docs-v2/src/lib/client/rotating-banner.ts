type BannerOption = {
  id: string
  src: string
  title: string
  artist: string
  year: string
  description: string
  objectPosition: string
}

type AstroBeforeSwapEvent = Event & {
  from: string | URL
  to: string | URL
  newDocument: Document
}

type AstroBeforePreparationEvent = AstroBeforeSwapEvent & {
  loader: () => Promise<void>
  signal: AbortSignal
}

type RotationAction = 'preserve' | 'next' | 'previous'
type BannerImageMode = 'direct' | 'preload'

type ApplyBannerOptions = {
  imageMode?: BannerImageMode
  persistSelection?: boolean
}

const BANNER_STORAGE_KEY = 'anchor-docs:banner-graphic-id'
const BASE_PATH = import.meta.env.BASE_URL.replace(/\/$/, '') || '/'
const BANNER_SELECTOR = '[data-doc-banner][data-rotating-banner="true"]'
const CONTROL_SELECTOR = '[data-banner-control]'

let listenersReady = false
let nextImageSwapToken = 0
const pendingImageSwaps = new WeakMap<Element, number>()

function isBannerOption(value: unknown): value is BannerOption {
  if (!value || typeof value !== 'object') return false

  const option = value as Partial<Record<keyof BannerOption, unknown>>
  return (
    typeof option.id === 'string' &&
    typeof option.src === 'string' &&
    typeof option.title === 'string' &&
    typeof option.artist === 'string' &&
    typeof option.year === 'string' &&
    typeof option.description === 'string' &&
    typeof option.objectPosition === 'string'
  )
}

function parseBannerOptions(value: string | null): BannerOption[] {
  if (!value) return []

  try {
    const parsed = JSON.parse(value)
    return Array.isArray(parsed) ? parsed.filter(isBannerOption) : []
  } catch {
    return []
  }
}

function readStoredGraphicId(): string {
  try {
    return sessionStorage.getItem(BANNER_STORAGE_KEY) ?? ''
  } catch {
    return ''
  }
}

function storeGraphicId(graphicId: string): void {
  try {
    sessionStorage.setItem(BANNER_STORAGE_KEY, graphicId)
  } catch {
    // Storage may be unavailable in strict privacy contexts.
  }
}

function selectedBanner(
  options: BannerOption[],
  currentGraphicId: string,
  action: RotationAction,
): BannerOption {
  const currentIndex = options.findIndex((option) => option.id === currentGraphicId)

  if (action === 'next') {
    return options[currentIndex >= 0 ? (currentIndex + 1) % options.length : 0]
  }

  if (action === 'previous') {
    return options[currentIndex >= 0 ? (currentIndex - 1 + options.length) % options.length : 0]
  }

  return options[currentIndex] ?? options[0]
}

function updateImageAttributes(image: HTMLImageElement, selected: BannerOption): void {
  image.alt = selected.description
  image.removeAttribute('title')
  image.style.objectPosition = selected.objectPosition
}

function waitForImageLoad(image: HTMLImageElement, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(new Error('Image preload aborted'))
      return
    }

    const cleanup = () => {
      image.removeEventListener('load', onLoad)
      image.removeEventListener('error', onError)
      signal?.removeEventListener('abort', onAbort)
    }
    const onLoad = () => {
      cleanup()
      resolve()
    }
    const onError = () => {
      cleanup()
      reject(new Error(`Unable to load ${image.src}`))
    }
    const onAbort = () => {
      cleanup()
      reject(new Error('Image preload aborted'))
    }

    image.addEventListener('load', onLoad, { once: true })
    image.addEventListener('error', onError, { once: true })
    signal?.addEventListener('abort', onAbort, { once: true })
  })
}

function abortable<T>(promise: Promise<T>, signal?: AbortSignal): Promise<T> {
  if (!signal) return promise
  if (signal.aborted) return Promise.reject(new Error('Image preload aborted'))

  return new Promise<T>((resolve, reject) => {
    const cleanup = () => signal.removeEventListener('abort', onAbort)
    const onAbort = () => {
      cleanup()
      reject(new Error('Image preload aborted'))
    }

    signal.addEventListener('abort', onAbort, { once: true })
    promise.then(
      (value) => {
        cleanup()
        resolve(value)
      },
      (error: unknown) => {
        cleanup()
        reject(error)
      },
    )
  })
}

async function preloadImage(src: string, signal?: AbortSignal): Promise<void> {
  const image = new Image()
  const loadPromise = waitForImageLoad(image, signal)

  image.decoding = 'async'
  image.src = src

  if (image.complete && image.naturalWidth > 0) return

  if (image.decode) {
    try {
      await abortable(image.decode(), signal)
      return
    } catch {
      // Some browsers reject decode() for images that still load successfully.
    }
  }

  await loadPromise
}

function updateBannerImage(banner: Element, selected: BannerOption): void {
  const image = banner.querySelector('[data-banner-image]')
  if (!(image instanceof HTMLImageElement)) return

  updateImageAttributes(image, selected)
  if (image.getAttribute('src') !== selected.src) {
    image.src = selected.src
  }
}

async function replaceBannerImage(banner: Element, selected: BannerOption): Promise<void> {
  const currentImage = banner.querySelector('[data-banner-image]')
  if (!(currentImage instanceof HTMLImageElement)) return

  if (currentImage.getAttribute('src') === selected.src) {
    updateImageAttributes(currentImage, selected)
    return
  }

  const imageSwapToken = ++nextImageSwapToken
  pendingImageSwaps.set(banner, imageSwapToken)

  try {
    await preloadImage(selected.src)
  } catch {
    return
  }

  if (pendingImageSwaps.get(banner) !== imageSwapToken) return

  const replacement = currentImage.ownerDocument.createElement('img')

  replacement.className = currentImage.className
  replacement.dataset.bannerImage = ''
  replacement.decoding = 'async'
  updateImageAttributes(replacement, selected)
  replacement.src = selected.src
  currentImage.replaceWith(replacement)
}

function updateBannerCaption(banner: Element, selected: BannerOption): void {
  const caption = banner.querySelector('[data-banner-caption]')

  if (!caption) return

  const title = caption.ownerDocument.createElement('cite')
  const details = [selected.artist, selected.year].filter(Boolean).join(', ')

  title.className = 'italic'
  title.textContent = selected.title
  caption.replaceChildren(title)
  if (details) caption.append(caption.ownerDocument.createTextNode(`, ${details}`))
}

function applyBanner(
  banner: Element,
  selected: BannerOption,
  { imageMode = 'preload', persistSelection = true }: ApplyBannerOptions = {},
): void {
  if (banner instanceof HTMLElement) {
    banner.dataset.bannerGraphicId = selected.id
  }

  if (imageMode === 'direct') {
    updateBannerImage(banner, selected)
  } else {
    void replaceBannerImage(banner, selected)
  }

  updateBannerCaption(banner, selected)
  if (persistSelection) {
    storeGraphicId(selected.id)
  }
}

function normalizedPath(url: string | URL): string {
  return new URL(url.toString(), window.location.href).pathname.replace(/\/+$/, '') || '/'
}

function isDocsHomeUrl(url: string | URL): boolean {
  return normalizedPath(url) === BASE_PATH
}

function shouldAdvanceBanner(fromUrl: string | URL, toUrl: string | URL): boolean {
  const fromPath = normalizedPath(fromUrl)
  const toPath = normalizedPath(toUrl)

  if (fromPath === toPath) return false
  return isDocsHomeUrl(fromUrl) === isDocsHomeUrl(toUrl)
}

export function setupRotatingDocBanners(
  root: ParentNode = document,
  currentGraphicId = '',
  action: RotationAction = 'preserve',
  options: ApplyBannerOptions = {},
): void {
  const banners = root.querySelectorAll(BANNER_SELECTOR)

  banners.forEach((banner) => {
    const bannerOptions = parseBannerOptions(banner.getAttribute('data-banner-options'))
    if (bannerOptions.length === 0) return

    const bannerGraphicId = banner instanceof HTMLElement ? banner.dataset.bannerGraphicId : ''
    const defaultGraphicId =
      banner instanceof HTMLElement ? (banner.dataset.bannerDefaultId ?? '') : ''
    const graphicId =
      currentGraphicId || bannerGraphicId || readStoredGraphicId() || defaultGraphicId
    applyBanner(banner, selectedBanner(bannerOptions, graphicId, action), options)
  })
}

async function prepareRotatingDocBanners(
  root: ParentNode,
  currentGraphicId: string,
  action: RotationAction,
  signal: AbortSignal,
): Promise<void> {
  const preloads: Promise<void>[] = []

  root.querySelectorAll(BANNER_SELECTOR).forEach((banner) => {
    const bannerOptions = parseBannerOptions(banner.getAttribute('data-banner-options'))
    if (bannerOptions.length === 0) return

    const bannerGraphicId = banner instanceof HTMLElement ? banner.dataset.bannerGraphicId : ''
    const defaultGraphicId =
      banner instanceof HTMLElement ? (banner.dataset.bannerDefaultId ?? '') : ''
    const graphicId =
      currentGraphicId || bannerGraphicId || readStoredGraphicId() || defaultGraphicId
    const selected = selectedBanner(bannerOptions, graphicId, action)

    applyBanner(banner, selected, { imageMode: 'direct', persistSelection: false })
    preloads.push(preloadImage(selected.src, signal).catch(() => undefined))
  })

  await Promise.all(preloads)
}

function updateBannerFromControl(control: HTMLElement): void {
  const banner = control.closest(BANNER_SELECTOR)
  if (!banner) return

  const action = control.dataset.bannerControl === 'previous' ? 'previous' : 'next'
  const bannerOptions = parseBannerOptions(banner.getAttribute('data-banner-options'))
  if (bannerOptions.length === 0) return

  const currentGraphicId = banner instanceof HTMLElement ? banner.dataset.bannerGraphicId : ''
  const defaultGraphicId =
    banner instanceof HTMLElement ? (banner.dataset.bannerDefaultId ?? '') : ''
  applyBanner(
    banner,
    selectedBanner(
      bannerOptions,
      currentGraphicId || readStoredGraphicId() || defaultGraphicId,
      action,
    ),
  )
}

function onControlClick(event: MouseEvent): void {
  const target = event.target instanceof Element ? event.target : null
  const control = target?.closest<HTMLElement>(CONTROL_SELECTOR)
  if (!control) return

  event.preventDefault()
  updateBannerFromControl(control)
}

function beforeSwap(event: Event): void {
  const { from, to, newDocument } = event as AstroBeforeSwapEvent
  const currentBanner = document.querySelector<HTMLElement>(BANNER_SELECTOR)
  const currentGraphicId = currentBanner?.dataset.bannerGraphicId ?? readStoredGraphicId()
  const action: RotationAction = shouldAdvanceBanner(from, to) ? 'next' : 'preserve'

  setupRotatingDocBanners(newDocument, currentGraphicId, action, { imageMode: 'direct' })
}

function beforePreparation(event: Event): void {
  const navigationEvent = event as AstroBeforePreparationEvent
  const currentBanner = document.querySelector<HTMLElement>(BANNER_SELECTOR)
  const currentGraphicId = currentBanner?.dataset.bannerGraphicId ?? readStoredGraphicId()
  const action: RotationAction = shouldAdvanceBanner(navigationEvent.from, navigationEvent.to)
    ? 'next'
    : 'preserve'
  const loadPage = navigationEvent.loader

  navigationEvent.loader = async () => {
    await loadPage()
    if (navigationEvent.signal.aborted) return

    await prepareRotatingDocBanners(
      navigationEvent.newDocument,
      currentGraphicId,
      action,
      navigationEvent.signal,
    )
  }
}

export function mountRotatingDocBanners(): void {
  setupRotatingDocBanners()

  if (listenersReady) return
  listenersReady = true

  document.addEventListener('click', onControlClick)
  document.addEventListener('astro:before-preparation', beforePreparation)
  document.addEventListener('astro:before-swap', beforeSwap)
  document.addEventListener('astro:after-swap', () => setupRotatingDocBanners())
}
