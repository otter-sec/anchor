import type { Element } from 'hast'
import { toHtml } from 'hast-util-to-html'
import { codeToHtml } from 'shiki'
import {
  CODE_ANNOTATION_TAGS,
  parseCodeAnnotations,
  stripCodeAnnotationTags,
} from './code-annotations'
import { darkTheme, lightTheme } from './shiki-themes'

const PATTERN = /`([^`]+?)(?:\{:([a-z0-9]+)\})?`/g

function escapeHtml(text: string): string {
  return text
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')
}

function inlineCodeHtml(code: string): string {
  const annotatedChildren = parseCodeAnnotations(code)
  if (annotatedChildren) {
    const node: Element = {
      type: 'element',
      tagName: 'code',
      properties: { className: ['annotated-code', 'code-with-tones'] },
      children: annotatedChildren,
    }
    return toHtml(node)
  }

  return `<code>${escapeHtml(code)}</code>`
}

function textHtml(text: string): string {
  const parts: string[] = []
  let lastIndex = 0
  let match: RegExpExecArray | null
  const annotationRe = new RegExp(`<(${CODE_ANNOTATION_TAGS.join('|')})>[\\s\\S]*?<\\/\\1>`, 'g')

  while ((match = annotationRe.exec(text)) !== null) {
    if (match.index > lastIndex) parts.push(escapeHtml(text.slice(lastIndex, match.index)))
    parts.push(inlineCodeHtml(match[0]))
    lastIndex = match.index + match[0].length
  }

  if (lastIndex < text.length) parts.push(escapeHtml(text.slice(lastIndex)))
  return parts.join('')
}

export interface RenderedDescription {
  html: string
  plain: string
}

export async function renderDescription(text: string): Promise<RenderedDescription> {
  const plain = stripCodeAnnotationTags(
    text.replace(PATTERN, (_, code) => stripCodeAnnotationTags(code)),
  )

  const re = new RegExp(PATTERN.source, 'g')
  const parts: string[] = []
  let lastIndex = 0
  let match: RegExpExecArray | null

  while ((match = re.exec(text)) !== null) {
    const [full, code, lang] = match
    if (match.index > lastIndex) {
      parts.push(textHtml(text.slice(lastIndex, match.index)))
    }
    if (lang === 'dir' || lang === 'file') {
      parts.push(
        `<code data-code-path-kind="${lang === 'dir' ? 'folder' : 'file'}">${escapeHtml(code)}</code>`,
      )
    } else if (lang) {
      const rendered = await codeToHtml(code, {
        lang,
        themes: { light: lightTheme, dark: darkTheme },
        structure: 'inline',
      })
      parts.push(`<span class="shiki">${rendered}</span>`)
    } else {
      parts.push(inlineCodeHtml(code))
    }
    lastIndex = match.index + full.length
  }

  if (lastIndex < text.length) {
    parts.push(textHtml(text.slice(lastIndex)))
  }

  return { html: parts.join(''), plain }
}
