let controller: AbortController | null = null
let lifecycleReady = false

function updateLandingScrollCue(cue: HTMLElement): void {
  const hidden = window.scrollY > 24
  cue.classList.toggle('pointer-events-none', hidden)
  cue.classList.toggle('translate-y-2', hidden)
  cue.classList.toggle('opacity-0', hidden)
}

function setupLandingScrollCue(): void {
  controller?.abort()
  controller = null

  const cue = document.querySelector<HTMLElement>('[data-scroll-cue]')
  if (!cue) return

  controller = new AbortController()
  const update = () => updateLandingScrollCue(cue)

  update()
  window.addEventListener('scroll', update, { passive: true, signal: controller.signal })
}

function cleanupLandingScrollCue(): void {
  controller?.abort()
  controller = null
}

export function mountLandingScrollCue(): void {
  setupLandingScrollCue()

  if (lifecycleReady) return
  lifecycleReady = true

  document.addEventListener('astro:before-swap', cleanupLandingScrollCue)
  document.addEventListener('astro:after-swap', setupLandingScrollCue)
}
