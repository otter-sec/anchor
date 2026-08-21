import { AttachedPluginData, definePlugin, type ExpressiveCodePlugin } from '@expressive-code/core'
import { h, select } from '@expressive-code/core/hast'
import { isTerminalLanguage } from './terminal-languages'

interface ShellPromptData {
  promptLines: Set<number>
}

const shellPromptData = new AttachedPluginData<ShellPromptData>(() => ({
  promptLines: new Set<number>(),
}))

export function pluginShellPrompts(): ExpressiveCodePlugin {
  return definePlugin({
    name: 'Shell Prompt',
    baseStyles: `
      .shell-prompt {
        color: var(--accent);
        font-weight: 500;
        margin-right: 1ch;
        user-select: none;
        -webkit-user-select: none;
      }
    `,
    hooks: {
      preprocessCode: ({ codeBlock }) => {
        if (codeBlock.props.frame !== 'terminal' && !isTerminalLanguage(codeBlock.language)) return

        const data = shellPromptData.getOrCreateFor(codeBlock)
        codeBlock.getLines().forEach((line, idx) => {
          if (line.text.startsWith('$ ')) {
            line.editText(0, 2, '')
            data.promptLines.add(idx)
          }
        })
      },
      postprocessRenderedLine: ({ codeBlock, lineIndex, renderData }) => {
        const data = shellPromptData.getOrCreateFor(codeBlock)
        if (!data.promptLines.has(lineIndex)) return

        const codeNode = select('div.code', renderData.lineAst)
        if (!codeNode) return
        const prompt = h('span', { class: 'shell-prompt', 'aria-hidden': 'true' }, '$')
        codeNode.children.unshift(prompt)
      },
    },
  })
}
