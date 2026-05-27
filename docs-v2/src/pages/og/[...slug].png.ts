import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { docSlugFromId, getAllDocs, type Doc } from '@/lib/docs'
import { darkTheme, lightTheme } from '@/lib/shiki-themes'
import type { MetaFile } from '@/types'
import { ImageResponse } from '@vercel/og'
import React from 'react'
import { codeToTokens } from 'shiki'
import type { BundledLanguage } from 'shiki'

const size = {
  width: 1200,
  height: 630,
}

const dmSansRegular = readFile(resolve(process.cwd(), 'src/assets/fonts/DMSans-Regular.ttf'))
const dmSansMedium = readFile(resolve(process.cwd(), 'src/assets/fonts/DMSans-Medium.ttf'))
const cascadiaCodeRegular = readFile(
  resolve(process.cwd(), 'src/assets/fonts/CascadiaCode-Regular.ttf'),
)
const wordmarkLight = readFile(resolve(process.cwd(), 'src/assets/wordmark-light.svg'), 'utf8')
const wordmarkDark = readFile(resolve(process.cwd(), 'src/assets/wordmark-dark.svg'), 'utf8')
const INLINE_CODE_PATTERN = /`([^`]+?)(?:\{:([a-z0-9]+)\})?`/g
const TRACKING_TIGHT = '-0.025em'

const metaModules = {
  ...(import.meta.glob('/src/content/docs/_meta.ts', {
    eager: true,
  }) as Record<string, { default: MetaFile }>),
  ...(import.meta.glob('/src/content/docs/**/_meta.ts', {
    eager: true,
  }) as Record<string, { default: MetaFile }>),
}

export async function getStaticPaths() {
  const docs = await getAllDocs()

  return docs.map((doc) => ({
    params: { slug: docSlugFromId(doc.id) ?? 'index' },
    props: { doc },
  }))
}

interface Props {
  doc: Doc
}

export async function GET({ props }: { props: Props }) {
  const { doc } = props
  const theme = resolveTheme(doc.id)
  const [title, description, wordmark, dmSansRegularData, dmSansMediumData, cascadiaCodeData] =
    await Promise.all([
      renderInlineText(doc.data.title, theme),
      doc.data.description ? renderInlineText(doc.data.description, theme) : null,
      svgDataUrl(theme.dark ? wordmarkDark : wordmarkLight),
      dmSansRegular,
      dmSansMedium,
      cascadiaCodeRegular,
    ])
  const breadcrumb = breadcrumbParts(doc.id)

  return new ImageResponse(
    React.createElement(
      'div',
      {
        style: {
          position: 'relative',
          width: '100%',
          height: '100%',
          display: 'flex',
          flexDirection: 'column',
          background: theme.background,
          color: theme.foreground,
          fontFamily: 'DM Sans',
          letterSpacing: TRACKING_TIGHT,
        },
      },
      React.createElement(
        'div',
        {
          style: {
            width: '100%',
            height: '100%',
            display: 'flex',
            flexDirection: 'column',
            justifyContent: 'space-between',
            padding: 64,
          },
        },
        React.createElement('img', {
          src: wordmark,
          alt: 'Anchor',
          style: {
            width: 164,
            height: 33,
            objectFit: 'contain',
          },
        }),
        React.createElement(
          'div',
          {
            style: {
              display: 'flex',
              flexDirection: 'column',
              maxWidth: 980,
            },
          },
          React.createElement(
            'div',
            {
              style: {
                display: 'flex',
                alignItems: 'center',
                color: theme.subtle,
                fontSize: 21,
                fontWeight: 500,
                lineHeight: 1,
                letterSpacing: TRACKING_TIGHT,
                marginBottom: 20,
              },
            },
            breadcrumb.flatMap((part, index) => [
              index > 0
                ? React.createElement(
                    'span',
                    {
                      key: `chevron-${index}`,
                      style: {
                        color: theme.subtle,
                        letterSpacing: TRACKING_TIGHT,
                        marginLeft: 11,
                        marginRight: 11,
                      },
                    },
                    '›',
                  )
                : null,
              React.createElement(
                'span',
                { key: `part-${index}`, style: { letterSpacing: TRACKING_TIGHT } },
                part,
              ),
            ]),
          ),
          React.createElement(
            'div',
            {
              style: {
                display: 'flex',
                flexWrap: 'wrap',
                fontSize: titleSize(title.plain),
                fontWeight: 500,
                lineHeight: 1.02,
                letterSpacing: TRACKING_TIGHT,
              },
            },
            title.nodes,
          ),
          description?.plain
            ? React.createElement(
                'div',
                {
                  style: {
                    display: 'flex',
                    flexWrap: 'wrap',
                    color: theme.muted,
                    fontSize: 28,
                    lineHeight: 1.23,
                    letterSpacing: TRACKING_TIGHT,
                    marginTop: 18,
                    maxWidth: 940,
                  },
                },
                description.nodes,
              )
            : null,
        ),
      ),
    ),
    {
      ...size,
      fonts: [
        { name: 'DM Sans', data: dmSansRegularData, weight: 400, style: 'normal' },
        { name: 'DM Sans', data: dmSansMediumData, weight: 500, style: 'normal' },
        { name: 'Cascadia Code', data: cascadiaCodeData, weight: 400, style: 'normal' },
      ],
    },
  )
}

async function svgDataUrl(svg: Promise<string>): Promise<string> {
  const data = await svg
  return `data:image/svg+xml;base64,${Buffer.from(data).toString('base64')}`
}

type OgTheme = ReturnType<typeof resolveTheme>

async function renderInlineText(
  text: string,
  theme: OgTheme,
): Promise<{ plain: string; nodes: React.ReactNode[] }> {
  const nodes: React.ReactNode[] = []
  const plain = text.replace(INLINE_CODE_PATTERN, (_, code) => code)
  const re = new RegExp(INLINE_CODE_PATTERN.source, 'g')
  let lastIndex = 0
  let index = 0
  let match: RegExpExecArray | null

  while ((match = re.exec(text)) !== null) {
    const [full, code, lang] = match
    if (match.index > lastIndex) {
      pushTextNodes(nodes, text.slice(lastIndex, match.index), index++)
    }

    nodes.push(
      await renderInlineCode(
        code,
        lang,
        theme,
        index++,
        /\s/.test(text[match.index + full.length] ?? ''),
      ),
    )
    lastIndex = match.index + full.length
  }

  if (lastIndex < text.length) {
    pushTextNodes(nodes, text.slice(lastIndex), index++)
  }

  return { plain, nodes }
}

function pushTextNodes(nodes: React.ReactNode[], text: string, baseIndex: number): void {
  const parts = text.match(/\S+\s*/g) ?? []
  parts.forEach((part, index) => {
    const word = part.trimEnd()
    const trailingSpace = word.length < part.length

    if (!word) return

    nodes.push(
      React.createElement(
        'span',
        {
          key: `text-${baseIndex}-${index}`,
          style: {
            letterSpacing: TRACKING_TIGHT,
            marginRight: trailingSpace ? '0.25em' : 0,
          },
        },
        word,
      ),
    )
  })
}

async function renderInlineCode(
  code: string,
  lang: string | undefined,
  theme: OgTheme,
  index: number,
  trailingSpace = false,
): Promise<React.ReactNode> {
  const codeStyle = {
    display: 'flex',
    alignItems: 'baseline',
    color: theme.codeForeground,
    background: theme.codeBackground,
    borderRadius: 5,
    padding: '0.03em 0.22em',
    fontFamily: 'Cascadia Code',
    fontSize: '0.88em',
    letterSpacing: TRACKING_TIGHT,
    marginRight: trailingSpace ? '0.25em' : 0,
    whiteSpace: 'pre',
  }

  if (!lang || lang === 'file' || lang === 'dir') {
    return React.createElement('span', { key: `code-${index}`, style: codeStyle }, code)
  }

  const highlighted = await codeToTokens(code, {
    lang: lang as BundledLanguage,
    theme: theme.dark ? darkTheme : lightTheme,
  })

  return React.createElement(
    'span',
    { key: `code-${index}`, style: codeStyle },
    highlighted.tokens.flat().map((token, tokenIndex) =>
      React.createElement(
        'span',
        {
          key: `token-${tokenIndex}`,
          style: {
            color: token.color ?? theme.codeForeground,
            letterSpacing: TRACKING_TIGHT,
            whiteSpace: 'pre',
          },
        },
        token.content,
      ),
    ),
  )
}

function resolveTheme(id: string) {
  const dark = id === 'v1' || id.startsWith('v1/')

  if (dark) {
    return {
      dark,
      background: '#191928',
      foreground: '#ced7f3',
      muted: '#a6adc8',
      subtle: '#a6adc8',
      border: '#303142',
      codeForeground: '#ced7f3',
      codeBackground: '#252535',
    }
  }

  return {
    dark,
    background: '#f1f1f7',
    foreground: '#4b5169',
    muted: '#6f7487',
    subtle: '#6f7487',
    border: '#dcddeb',
    codeForeground: '#4b5169',
    codeBackground: '#e7e7f1',
  }
}

function titleSize(title: string): number {
  if (title.length > 70) return 50
  if (title.length > 52) return 56
  if (title.length > 34) return 62
  return 70
}

function breadcrumbParts(id: string): string[] {
  if (id === 'index') return ['Docs']

  const parts = id
    .replace(/\/index$/, '')
    .split('/')
    .filter(Boolean)

  if (parts.length === 0) return ['Docs']

  return parts.slice(0, 2).map((_, index) => breadcrumbLabel(parts, index))
}

function breadcrumbLabel(parts: string[], index: number): string {
  const part = parts[index]

  if (part === 'v1' || part === 'v2') return part

  const parentPath = parts.slice(0, index).join('/')
  const currentPath = parts.slice(0, index + 1).join('/')
  return (
    metaFor(parentPath).items?.[part]?.label ?? metaFor(currentPath).label ?? sentenceCase(part)
  )
}

function metaFor(dirPath: string): MetaFile {
  const key = dirPath ? `/src/content/docs/${dirPath}/_meta.ts` : '/src/content/docs/_meta.ts'
  return metaModules[key]?.default ?? {}
}

function sentenceCase(input: string): string {
  const text = input.split('-').filter(Boolean).join(' ')

  return text.charAt(0).toUpperCase() + text.slice(1)
}
