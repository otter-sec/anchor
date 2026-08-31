import { definePlugin, type ExpressiveCodePlugin } from '@expressive-code/core'
import { select, selectAll, type Element, type ElementContent } from '@expressive-code/core/hast'
import { isTerminalLanguage } from './terminal-languages'

function addClass(node: Element, className: string): void {
  const classes = node.properties?.className
  const arr = Array.isArray(classes) ? [...classes] : []
  if (!arr.includes(className)) arr.push(className)
  node.properties = { ...node.properties, className: arr }
}

function extractText(node: ElementContent): string {
  if (node.type === 'text') return node.value
  if (node.type !== 'element' || !node.children) return ''
  return node.children.map(extractText).join('')
}

function extractCommandText(codeNode: Element): string {
  const parts: string[] = []
  for (const child of codeNode.children) {
    if (
      child.type === 'element' &&
      Array.isArray(child.properties?.className) &&
      child.properties.className.includes('shell-prompt')
    ) {
      continue
    }
    parts.push(extractText(child))
  }
  return parts.join('')
}

export function pluginOutputSeparators(): ExpressiveCodePlugin {
  return definePlugin({
    name: 'Output Separator',
    baseStyles: `
      .ec-line.ec-cmd + .ec-line.ec-out,
      .ec-line.ec-out + .ec-line.ec-cmd {
        border-top: 2px solid color-mix(in oklab, var(--border) 75%, transparent);
        margin-top: 0.75rem;
        padding-top: 0.75rem;
      }
    `,
    hooks: {
      postprocessRenderedBlock: ({ codeBlock, renderData }) => {
        if (!isTerminalLanguage(codeBlock.language)) return

        const lines = selectAll('div.ec-line', renderData.blockAst)
        if (lines.length === 0) return

        const commands: string[] = []
        let hasCmd = false
        let hasOut = false
        let inContinuation = false

        for (const line of lines) {
          const codeNode = select('div.code', line)
          const hasPrompt = !!select('span.shell-prompt', line)
          const isCmd = hasPrompt || inContinuation

          if (isCmd) {
            addClass(line, 'ec-cmd')
            hasCmd = true
            if (codeNode) {
              const text = extractCommandText(codeNode)
              if (hasPrompt || commands.length === 0) {
                commands.push(text)
              } else {
                commands[commands.length - 1] += '\n' + text
              }
            }
            const lineText = codeNode ? extractCommandText(codeNode) : ''
            inContinuation = /\\\s*$/.test(lineText)
          } else {
            addClass(line, 'ec-out')
            hasOut = true
            inContinuation = false
          }
        }

        if (!hasCmd || !hasOut) return

        const copyButton = select('.copy button', renderData.blockAst)
        if (copyButton) {
          copyButton.properties = {
            ...copyButton.properties,
            'data-code': commands.join('\u007f'),
          }
        }
      },
    },
  })
}
