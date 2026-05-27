import { rehypeHeadingIds } from '@astrojs/markdown-remark'
import mdx from '@astrojs/mdx'
import react from '@astrojs/react'
import sitemap from '@astrojs/sitemap'
import rehypeShiki from '@shikijs/rehype'
import tailwindcss from '@tailwindcss/vite'
import pagefind from 'astro-pagefind'
import { defineConfig } from 'astro/config'
import { readFile } from 'node:fs/promises'
import { extname, isAbsolute, relative, resolve } from 'node:path'
import rehypeAutolinkHeadings from 'rehype-autolink-headings'
import rehypeExpressiveCode from 'rehype-expressive-code'
import rehypeExternalLinks from 'rehype-external-links'
import rehypeKatex from 'rehype-katex'
import remarkEmoji from 'remark-emoji'
import remarkMath from 'remark-math'
import { expressiveCodeOptions } from './src/lib/expressive-code-config'
import { rehypeCodeAnnotations } from './src/lib/rehype-code-annotations'
import { rehypeCodePathHints, rehypeCodePathIcons } from './src/lib/rehype-code-paths'
import { rehypeCopyableShellCommands } from './src/lib/rehype-copyable-shell-commands'
import { rehypeLinkIcons } from './src/lib/rehype-link-icons'
import { rehypeTableWrappers } from './src/lib/rehype-table-wrappers'
import { darkTheme, lightTheme } from './src/lib/shiki-themes'

type DevMiddleware = (
  req: { url?: string },
  res: { setHeader(name: string, value: string): void; end(data: Uint8Array): void },
  next: () => void,
) => void | Promise<void>

type DevServer = {
  middlewares: {
    use(path: string, handler: DevMiddleware): void
  }
}

const DOCS_BASE = '/docs'
const PAGEFIND_DIST_DIR = resolve(process.cwd(), 'dist', 'docs', 'pagefind')

function pagefindDevServer() {
  const mime: Record<string, string> = {
    js: 'application/javascript',
    mjs: 'application/javascript',
    css: 'text/css',
    json: 'application/json',
    wasm: 'application/wasm',
  }

  return {
    name: 'pagefind-dev-server',
    enforce: 'pre' as const,
    apply: 'serve' as const,
    configureServer(server: DevServer) {
      server.middlewares.use('/pagefind', async (req, res, next) => {
        const filePath = resolvePagefindAsset(req.url ?? '/')
        if (!filePath) return next()

        try {
          const data = await readFile(filePath)
          const ext = extname(filePath).slice(1)
          if (mime[ext]) res.setHeader('Content-Type', mime[ext])
          res.end(data)
        } catch {
          next()
        }
      })
    },
  }
}

function resolvePagefindAsset(url: string): string | null {
  let pathname: string
  try {
    pathname = decodeURIComponent(url.split('?')[0] ?? '/')
  } catch {
    return null
  }

  if (!pathname || pathname === '/') return null

  const filePath = resolve(PAGEFIND_DIST_DIR, `.${pathname}`)
  const relativePath = relative(PAGEFIND_DIST_DIR, filePath)

  if (relativePath.startsWith('..') || isAbsolute(relativePath)) return null

  return filePath
}

export default defineConfig({
  site: 'https://www.anchor-lang.com',
  base: DOCS_BASE,
  trailingSlash: 'always',
  outDir: './dist/docs',
  integrations: [mdx(), react(), sitemap(), pagefind()],
  vite: {
    plugins: [tailwindcss(), pagefindDevServer()],
  },
  server: {
    port: 4321,
    host: true,
  },
  devToolbar: {
    enabled: false,
  },
  markdown: {
    syntaxHighlight: false,
    rehypePlugins: [
      [
        rehypeExternalLinks,
        {
          target: '_blank',
          rel: ['nofollow', 'noreferrer', 'noopener'],
        },
      ],
      rehypeTableWrappers,
      rehypeKatex,
      [rehypeExpressiveCode, { themes: [lightTheme, darkTheme], ...expressiveCodeOptions }],
      rehypeCodePathHints,
      [
        rehypeShiki,
        {
          themes: { light: lightTheme, dark: darkTheme },
          inline: 'tailing-curly-colon',
        },
      ],
      rehypeCopyableShellCommands,
      rehypeCodeAnnotations,
      rehypeCodePathIcons,
      rehypeLinkIcons,
      rehypeHeadingIds,
      [
        rehypeAutolinkHeadings,
        {
          behavior: 'append',
          properties: {
            className: ['heading-anchor'],
            'aria-label': 'Link to section',
            tabindex: -1,
            'data-pagefind-ignore': '',
          },
          content: {
            type: 'text',
            value: '#',
          },
          test: (node: { tagName: string }) =>
            ['h2', 'h3', 'h4', 'h5', 'h6'].includes(node.tagName),
        },
      ],
    ],
    remarkPlugins: [remarkMath, remarkEmoji],
  },
})
