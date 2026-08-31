import { mountClientModule } from './lifecycle'

const TAB_ROOT_SELECTOR = '[data-tabs]'
const TAB_PANEL_SELECTOR = '[data-tab-panel]'

type SelectTabs = (index: number, options?: SelectOptions) => void
type SelectOptions = {
  syncNestedPanels: boolean
  syncPeers: boolean
}

const selectByRoot = new WeakMap<HTMLElement, SelectTabs>()

function clampIndex(index: number, length: number): number {
  if (!Number.isFinite(index)) return 0
  return Math.max(0, Math.min(index, Math.max(0, length - 1)))
}

function directTabButtons(root: HTMLElement): HTMLButtonElement[] {
  const tabList = Array.from(root.children).find(
    (child): child is HTMLElement =>
      child instanceof HTMLElement && child.getAttribute('role') === 'tablist',
  )

  if (!tabList) return []

  return Array.from(tabList.children).filter(
    (child): child is HTMLButtonElement =>
      child instanceof HTMLButtonElement && child.hasAttribute('data-tab-index'),
  )
}

function directTabPanels(panelsWrapper: HTMLElement): HTMLElement[] {
  return Array.from(panelsWrapper.children).filter(
    (panel): panel is HTMLElement =>
      panel instanceof HTMLElement && panel.matches(TAB_PANEL_SELECTOR),
  )
}

function directPanelsWrapper(root: HTMLElement): HTMLElement | null {
  return (
    Array.from(root.children).find(
      (child): child is HTMLElement =>
        child instanceof HTMLElement && child.hasAttribute('data-tabs-panels'),
    ) ?? null
  )
}

function directNestedTabs(panel: HTMLElement): HTMLElement[] {
  return Array.from(panel.children).filter(
    (child): child is HTMLElement =>
      child instanceof HTMLElement && child.matches(TAB_ROOT_SELECTOR),
  )
}

function syncGroupPeers(root: HTMLElement): HTMLElement[] {
  const group = root.dataset.syncGroup
  if (!group) return []
  return Array.from(
    document.querySelectorAll<HTMLElement>(`[data-tabs][data-sync-group="${CSS.escape(group)}"]`),
  ).filter((peer) => peer !== root)
}

function peerNestedTabs(root: HTMLElement): HTMLElement[] {
  const panel = root.parentElement?.closest<HTMLElement>(TAB_PANEL_SELECTOR)
  const panelsWrapper = panel?.parentElement
  if (!panel || !(panelsWrapper instanceof HTMLElement)) return []
  if (!panelsWrapper.hasAttribute('data-tabs-panels')) return []

  const nestedTabs = directNestedTabs(panel)
  const rootIndex = nestedTabs.indexOf(root)
  if (rootIndex === -1) return []

  return Array.from(panelsWrapper.children).flatMap((child) => {
    if (!(child instanceof HTMLElement) || child === panel) return []
    if (!child.matches(TAB_PANEL_SELECTOR)) return []
    const peer = directNestedTabs(child)[rootIndex]
    return peer ? [peer] : []
  })
}

function indexForKey(tabs: HTMLButtonElement[], key: string): number | null {
  const index = tabs.findIndex((tab) => tab.dataset.tabKey === key)
  return index === -1 ? null : index
}

function selectRootByKey(root: HTMLElement, key: string): void {
  const tabs = directTabButtons(root)
  const index = indexForKey(tabs, key)
  const select = selectByRoot.get(root)

  if (index === null || !select) return
  select(index, { syncNestedPanels: false, syncPeers: false })
}

function syncNestedPanels(from: HTMLElement | undefined, to: HTMLElement | undefined): void {
  if (!from || !to || from === to) return

  directNestedTabs(from).forEach((source, index) => {
    const key = source.dataset.selectedTabKey
    const target = directNestedTabs(to)[index]

    if (!key || !target) return
    selectRootByKey(target, key)
  })
}

function setupTabRoot(root: HTMLElement): void {
  if (root.dataset.tabsInit === '1') return
  root.dataset.tabsInit = '1'

  const tabs = directTabButtons(root)
  const panelsWrapper = directPanelsWrapper(root)
  if (!panelsWrapper) return

  const panels = directTabPanels(panelsWrapper)
  const defaultIndex = clampIndex(Number(panelsWrapper.dataset.defaultIndex ?? 0), panels.length)

  const select = (
    index: number,
    options: SelectOptions = { syncNestedPanels: true, syncPeers: true },
  ) => {
    const selectedIndex = clampIndex(index, panels.length)
    const previousIndex = clampIndex(
      Number(root.dataset.selectedTabIndex ?? defaultIndex),
      panels.length,
    )
    const selectedTab = tabs[selectedIndex]
    const selectedKey = selectedTab?.dataset.tabKey ?? String(selectedIndex)
    const previousPanel = panels[previousIndex]
    const selectedPanel = panels[selectedIndex]

    root.dataset.selectedTabIndex = String(selectedIndex)
    root.dataset.selectedTabKey = selectedKey

    tabs.forEach((tab, i) => {
      const active = i === selectedIndex
      tab.setAttribute('aria-selected', String(active))
      tab.dataset.selected = String(active)
      tab.tabIndex = active ? 0 : -1
    })

    panels.forEach((panel, i) => {
      panel.hidden = i !== selectedIndex
    })

    if (options.syncNestedPanels) syncNestedPanels(previousPanel, selectedPanel)
    if (!options.syncPeers) return

    const peers = [...peerNestedTabs(root), ...syncGroupPeers(root)]
    peers.forEach((peer) => {
      const peerTabs = directTabButtons(peer)
      const peerIndex = indexForKey(peerTabs, selectedKey)
      const selectPeer = selectByRoot.get(peer)

      if (peerIndex === null || !selectPeer) return
      selectPeer(peerIndex, { syncNestedPanels: false, syncPeers: false })
    })
  }

  selectByRoot.set(root, select)
  select(defaultIndex, { syncNestedPanels: false, syncPeers: false })

  tabs.forEach((tab, index) => {
    tab.addEventListener('click', () => select(index))
    tab.addEventListener('keydown', (event) => {
      const isRight = event.key === 'ArrowRight'
      const isLeft = event.key === 'ArrowLeft'
      if (!isRight && !isLeft) return

      event.preventDefault()
      const tabId = (isRight ? index + 1 : index - 1 + tabs.length) % tabs.length
      tabs[tabId]?.focus()
      select(tabId)
    })
  })
}

function resetTabs(): void {
  document.querySelectorAll<HTMLElement>(TAB_ROOT_SELECTOR).forEach((root) => {
    delete root.dataset.tabsInit
  })
}

export function setupTabs(): void {
  document.querySelectorAll<HTMLElement>(TAB_ROOT_SELECTOR).forEach(setupTabRoot)
}

export const mountTabs = mountClientModule({
  setup: () => {
    resetTabs()
    setupTabs()
  },
})
