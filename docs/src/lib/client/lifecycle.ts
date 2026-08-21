/**
 * Wires a client-side module into Astro's view-transition lifecycle.
 *
 * `astro:page-load` fires on initial page load *and* after every CSN
 * transition (after stylesheets and scripts have loaded). That's the
 * documented place to run anything you'd otherwise run on
 * `DOMContentLoaded`. Cleanup runs on `astro:before-swap` so handlers
 * are torn down before the new DOM is mounted.
 *
 * The returned mount function is idempotent: calling it more than once
 * is a no-op, so it's safe to call from a bundled module `<script>`
 * that may execute on initial load only.
 */
export function mountClientModule(opts: {
  setup: () => void
  cleanup?: (event: Event) => void
  /** Runs once on first mount - for delegated listeners that survive navigation. */
  initOnce?: () => void
}): () => void {
  let ready = false
  return () => {
    if (ready) return
    ready = true
    opts.initOnce?.()
    if (opts.cleanup) document.addEventListener('astro:before-swap', opts.cleanup)
    document.addEventListener('astro:page-load', opts.setup)
  }
}
