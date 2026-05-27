import { mountClientModule } from './lifecycle'

let controller: AbortController | null = null

function setupMobileSidebar(): void {
  const dialog = document.getElementById('mobile-sidebar') as HTMLDialogElement | null
  const trigger = document.getElementById('mobile-sidebar-trigger') as HTMLButtonElement | null
  const closeButton = document.getElementById('mobile-sidebar-close') as HTMLButtonElement | null

  if (!dialog) return

  controller?.abort()
  controller = new AbortController()
  const { signal } = controller

  const open = () => {
    if (!dialog.open) dialog.showModal()
    trigger?.setAttribute('aria-expanded', 'true')
  }

  const close = () => {
    if (dialog.open) dialog.close()
    trigger?.setAttribute('aria-expanded', 'false')
  }

  trigger?.addEventListener('click', open, { signal })
  closeButton?.addEventListener('click', close, { signal })

  dialog.addEventListener(
    'click',
    (event) => {
      if (event.target === dialog) close()
    },
    { signal },
  )

  dialog.addEventListener(
    'close',
    () => {
      trigger?.setAttribute('aria-expanded', 'false')
    },
    { signal },
  )

  dialog
    .querySelectorAll<HTMLAnchorElement>('a[href]')
    .forEach((link) => link.addEventListener('click', close, { signal }))
}

export const mountMobileSidebar = mountClientModule({
  setup: setupMobileSidebar,
  cleanup: () => controller?.abort(),
})
