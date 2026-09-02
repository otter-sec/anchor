import { mountClientModule } from './lifecycle'

async function ensureKatexStyles(): Promise<void> {
  if (!document.querySelector('.katex') || document.querySelector('link[data-katex-stylesheet]')) {
    return
  }

  // Importing KaTeX CSS as a Vite URL keeps it version-locked with the
  // package dependency and avoids any CDN dependency.
  const { default: href } = await import('katex/dist/katex.min.css?url')
  const link = document.createElement('link')
  link.rel = 'stylesheet'
  link.href = href
  link.dataset.katexStylesheet = 'true'
  document.head.appendChild(link)
}

export const mountKatexStyles = mountClientModule({
  setup: () => void ensureKatexStyles(),
})
