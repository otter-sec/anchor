export function trimTrailingSlash(path: string): string {
  if (path === '/') return path
  return path.replace(/\/+$/, '')
}

export function findScrollAreaViewport(root: ParentNode | null | undefined): HTMLElement | null {
  return (
    root?.querySelector<HTMLElement>(
      '[data-slot="scroll-area-viewport"], [data-radix-scroll-area-viewport]',
    ) ?? null
  )
}

export function isElementHidden(element: HTMLElement): boolean {
  return Boolean(element.closest('[hidden]')) || element.getClientRects().length === 0
}

export function withDatasetFlag(selector: string, flag: string, callback: () => void): void {
  const roots = Array.from(document.querySelectorAll<HTMLElement>(selector))

  roots.forEach((root) => {
    root.dataset[flag] = 'true'
  })

  try {
    callback()
  } finally {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        roots.forEach((root) => {
          delete root.dataset[flag]
        })
      })
    })
  }
}
