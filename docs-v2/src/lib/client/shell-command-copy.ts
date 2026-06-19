import { mountClientModule } from './lifecycle'

async function copyShellCommand(button: HTMLElement): Promise<void> {
  if (button.dataset.copying === '1') return

  const command = button.getAttribute('data-copy')
  if (!command) return

  try {
    await navigator.clipboard.writeText(command)
  } catch {
    return
  }

  button.dataset.copying = '1'
  const prompt = button.querySelector<HTMLSpanElement>('.shell-prompt')
  const originalPrompt = prompt?.textContent ?? '$'

  if (prompt) prompt.textContent = '✓'
  button.classList.add('copied')

  window.setTimeout(() => {
    if (prompt) prompt.textContent = originalPrompt
    button.classList.remove('copied')
    delete button.dataset.copying
  }, 1200)
}

function shellCommandFromEvent(event: Event): HTMLElement | null {
  const target = event.target instanceof HTMLElement ? event.target : null
  return target?.closest<HTMLElement>('.copyable-shell-command') ?? null
}

function handleClick(event: MouseEvent): void {
  const button = shellCommandFromEvent(event)
  if (button) void copyShellCommand(button)
}

function handleKeydown(event: KeyboardEvent): void {
  if (event.key !== 'Enter' && event.key !== ' ') return

  const button = shellCommandFromEvent(event)
  if (!button) return

  event.preventDefault()
  void copyShellCommand(button)
}

// Delegated click/keydown listeners on `document` survive CSN navigations,
// so there's nothing to set up per page.
export const mountShellCommandCopy = mountClientModule({
  setup: () => {},
  initOnce: () => {
    document.addEventListener('click', handleClick)
    document.addEventListener('keydown', handleKeydown)
  },
})
