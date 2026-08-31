import { cn } from '@/lib/utils'
import { ScrollArea as ScrollAreaPrimitive } from 'radix-ui'
import * as React from 'react'

type ScrollAreaOrientation = 'vertical' | 'horizontal' | 'both'

type ScrollAreaProps = React.ComponentProps<typeof ScrollAreaPrimitive.Root> & {
  fades?: boolean
  fadeOrientation?: ScrollAreaOrientation
  scrollbars?: ScrollAreaOrientation
  viewportClassName?: string
}

type ScrollAreaFadeState = {
  top: boolean
  bottom: boolean
  left: boolean
  right: boolean
}

type ScrollAreaFadesProps = {
  fadeState: ScrollAreaFadeState
  showHorizontalFades: boolean
  showHorizontalScrollbar: boolean
  showVerticalFades: boolean
  showVerticalScrollbar: boolean
}

function ScrollArea({
  className,
  fades = false,
  fadeOrientation = 'vertical',
  scrollbars = 'vertical',
  viewportClassName,
  children,
  type = 'scroll',
  ...props
}: ScrollAreaProps) {
  const viewportRef = React.useRef<React.ComponentRef<typeof ScrollAreaPrimitive.Viewport>>(null)
  const [fadeState, setFadeState] = React.useState<ScrollAreaFadeState>({
    top: false,
    bottom: false,
    left: false,
    right: false,
  })
  const showVerticalFades = fades && (fadeOrientation === 'vertical' || fadeOrientation === 'both')
  const showHorizontalFades =
    fades && (fadeOrientation === 'horizontal' || fadeOrientation === 'both')
  const showVerticalScrollbar = scrollbars === 'vertical' || scrollbars === 'both'
  const showHorizontalScrollbar = scrollbars === 'horizontal' || scrollbars === 'both'

  const updateFadeState = React.useCallback(() => {
    const viewport = viewportRef.current
    if (!viewport || !fades) return

    const maxScrollTop = viewport.scrollHeight - viewport.clientHeight
    const maxScrollLeft = viewport.scrollWidth - viewport.clientWidth
    const next = {
      top: showVerticalFades && maxScrollTop > 1 && viewport.scrollTop > 1,
      bottom: showVerticalFades && maxScrollTop > 1 && viewport.scrollTop < maxScrollTop - 1,
      left: showHorizontalFades && maxScrollLeft > 1 && viewport.scrollLeft > 1,
      right: showHorizontalFades && maxScrollLeft > 1 && viewport.scrollLeft < maxScrollLeft - 1,
    }

    setFadeState((current) =>
      current.top === next.top &&
      current.bottom === next.bottom &&
      current.left === next.left &&
      current.right === next.right
        ? current
        : next,
    )
  }, [fades, showHorizontalFades, showVerticalFades])

  React.useEffect(() => {
    if (!fades) return

    const viewport = viewportRef.current
    if (!viewport) return

    updateFadeState()
    viewport.addEventListener('scroll', updateFadeState, { passive: true })

    const observer = new ResizeObserver(updateFadeState)
    observer.observe(viewport)
    if (viewport.firstElementChild) observer.observe(viewport.firstElementChild)

    return () => {
      viewport.removeEventListener('scroll', updateFadeState)
      observer.disconnect()
    }
  }, [fades, updateFadeState])

  return (
    <ScrollAreaPrimitive.Root
      data-slot="scroll-area"
      type={type}
      scrollHideDelay={250}
      className={cn('relative', className)}
      {...props}
    >
      <ScrollAreaPrimitive.Viewport
        ref={viewportRef}
        data-slot="scroll-area-viewport"
        className={cn(
          'ring-ring/10 dark:ring-ring/20 dark:outline-ring/40 outline-ring/50 size-full',
          'rounded-[inherit] transition-[color,box-shadow]',
          'focus-visible:ring-4 focus-visible:outline-1',
          viewportClassName,
        )}
      >
        {children}
      </ScrollAreaPrimitive.Viewport>
      <ScrollAreaFades
        fadeState={fadeState}
        showHorizontalFades={showHorizontalFades}
        showHorizontalScrollbar={showHorizontalScrollbar}
        showVerticalFades={showVerticalFades}
        showVerticalScrollbar={showVerticalScrollbar}
      />
      {showVerticalScrollbar && <ScrollBar />}
      {showHorizontalScrollbar && <ScrollBar orientation="horizontal" />}
      <ScrollAreaPrimitive.Corner />
    </ScrollAreaPrimitive.Root>
  )
}

function ScrollAreaFades({
  fadeState,
  showHorizontalFades,
  showHorizontalScrollbar,
  showVerticalFades,
  showVerticalScrollbar,
}: ScrollAreaFadesProps) {
  return (
    <>
      {showVerticalFades && (
        <>
          <div
            aria-hidden="true"
            data-slot="scroll-area-fade-top"
            className={cn(
              'from-background pointer-events-none absolute top-0 left-0 z-10 h-8',
              showVerticalScrollbar ? 'right-2.5' : 'right-0',
              'bg-linear-to-b to-transparent transition-opacity',
              fadeState.top ? 'opacity-100' : 'opacity-0',
            )}
          />
          <div
            aria-hidden="true"
            data-slot="scroll-area-fade-bottom"
            className={cn(
              'from-background pointer-events-none absolute bottom-0 left-0 z-10 h-8',
              showVerticalScrollbar ? 'right-2.5' : 'right-0',
              'bg-linear-to-t to-transparent transition-opacity',
              fadeState.bottom ? 'opacity-100' : 'opacity-0',
            )}
          />
        </>
      )}
      {showHorizontalFades && (
        <>
          <div
            aria-hidden="true"
            data-slot="scroll-area-fade-left"
            className={cn(
              'from-background pointer-events-none absolute top-0 left-0 z-10 w-8',
              showHorizontalScrollbar ? 'bottom-2.5' : 'bottom-0',
              'bg-linear-to-r to-transparent transition-opacity',
              fadeState.left ? 'opacity-100' : 'opacity-0',
            )}
          />
          <div
            aria-hidden="true"
            data-slot="scroll-area-fade-right"
            className={cn(
              'from-background pointer-events-none absolute top-0 right-0 z-10 w-8',
              showHorizontalScrollbar ? 'bottom-2.5' : 'bottom-0',
              'bg-linear-to-l to-transparent transition-opacity',
              fadeState.right ? 'opacity-100' : 'opacity-0',
            )}
          />
        </>
      )}
    </>
  )
}

function ScrollBar({
  className,
  orientation = 'vertical',
  ...props
}: React.ComponentProps<typeof ScrollAreaPrimitive.ScrollAreaScrollbar>) {
  return (
    <ScrollAreaPrimitive.ScrollAreaScrollbar
      data-slot="scroll-area-scrollbar"
      orientation={orientation}
      className={cn(
        'flex touch-none p-px transition-colors select-none',
        orientation === 'vertical' && 'h-full w-2.5 border-l border-l-transparent',
        orientation === 'horizontal' && 'h-2.5 flex-col border-t border-t-transparent',
        className,
      )}
      {...props}
    >
      <ScrollAreaPrimitive.ScrollAreaThumb
        data-slot="scroll-area-thumb"
        className="bg-border relative flex-1 rounded-full"
      />
    </ScrollAreaPrimitive.ScrollAreaScrollbar>
  )
}

export { ScrollArea, ScrollBar }
