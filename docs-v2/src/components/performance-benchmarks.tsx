import { useEffect, useRef, useState, type RefObject } from 'react'
import { bindHorizontalScrollFade } from '@/lib/client/horizontal-scroll-fade'
import {
  benchmarkTabs,
  programs,
  type ActiveBenchmarkTab,
  type BenchmarkMetric,
  type ProgramBenchmark,
} from '@/lib/landing-benchmarks'

const benchmarkBaselineColor = 'bg-[color-mix(in_oklch,var(--ctp-overlay-0)_76%,transparent)]'
const benchmarkBaselineColorDark =
  'dark:bg-[color-mix(in_oklch,var(--ctp-overlay-0)_68%,transparent)]'
const tabularNums = '[font-feature-settings:"tnum"_1] [font-variant-numeric:tabular-nums]'

function BenchmarkMeter({ row }: { row: BenchmarkMetric }) {
  return (
    <div className="min-w-0 self-end [grid-area:meter]">
      <div
        className={`${benchmarkBaselineColor} ${benchmarkBaselineColorDark} relative h-3 min-w-0 overflow-hidden rounded-full`}
        aria-hidden="true"
      >
        <span
          className="bg-accent absolute inset-y-0 left-0 min-w-2.5 rounded-[inherit]"
          style={{ width: `${Math.max(row.v2Share, 1.5)}%` }}
        />
      </div>
      <div
        className={`text-muted-foreground mt-3 grid min-w-0 grid-cols-2 gap-2 text-[0.8125rem] leading-[1.2] ${tabularNums}`}
      >
        <span className="flex min-w-0 flex-col gap-0.5">
          <span className="text-muted-foreground/75 text-[0.6875rem] leading-none">v1</span>
          {row.olderLabel}
        </span>
        <span className="flex min-w-0 flex-col gap-0.5">
          <span className="text-muted-foreground/75 text-[0.6875rem] leading-none">v2</span>
          {row.v2Label}
        </span>
      </div>
    </div>
  )
}

function ReductionMetric({ row }: { row: BenchmarkMetric }) {
  return (
    <div className="min-w-0 text-right [grid-area:reduction]">
      <div
        className={`text-foreground text-2xl leading-none font-medium whitespace-nowrap ${tabularNums}`}
      >
        {row.reductionLabel}
      </div>
      <div className="text-muted-foreground mt-1.5 text-[0.8125rem] leading-[1.1] whitespace-nowrap">
        {row.changeLabel}
      </div>
    </div>
  )
}

function ProgramSummary({ row }: { row: BenchmarkMetric }) {
  return (
    <div className="grid w-full min-w-0 grid-cols-[minmax(0,1fr)_max-content] grid-rows-[auto_1fr] gap-x-4 gap-y-5 [grid-template-areas:'text_reduction'_'meter_meter']">
      <div className="min-w-0 [grid-area:text]">
        <div className="text-foreground overflow-hidden text-base leading-[1.2] font-medium text-ellipsis whitespace-nowrap">
          {row.label}
        </div>
        <div className="text-muted-foreground mt-1.5 overflow-hidden text-sm leading-[1.3] [display:-webkit-box] [-webkit-box-orient:vertical] [-webkit-line-clamp:2]">
          {row.detail}
        </div>
      </div>

      <BenchmarkMeter row={row} />

      <ReductionMetric row={row} />
    </div>
  )
}

function ProgramRow({
  program,
  activeTab,
}: {
  program: ProgramBenchmark
  activeTab: ActiveBenchmarkTab
}) {
  const row = activeTab === 'binary' ? program.binary : program.compute

  return (
    <div
      className="border-border/80 bg-background flex min-h-[11.5rem] min-w-0 flex-[0_0_min(24rem,calc(100vw-2rem))] snap-start rounded-[1.35rem] border-2 p-[1.125rem] sm:min-h-[12.25rem] sm:flex-basis-[25.5rem] lg:flex-basis-[27rem] xl:flex-basis-[25.5rem]"
      data-benchmark-row
    >
      <ProgramSummary row={row} />
    </div>
  )
}

function BenchmarkPanel({
  activeTab,
  scrollRef,
}: {
  activeTab: ActiveBenchmarkTab
  scrollRef: RefObject<HTMLDivElement | null>
}) {
  return (
    <div
      id={`benchmark-${activeTab}-panel`}
      role="tabpanel"
      aria-labelledby={`benchmark-${activeTab}-tab`}
      className="mt-3 w-full"
    >
      <div
        className="relative mt-3 -mx-2 w-[calc(100%+1rem)] before:pointer-events-none before:absolute before:top-0 before:bottom-3 before:left-0 before:z-10 before:w-10 before:bg-gradient-to-r before:from-background before:to-transparent before:opacity-0 before:transition-opacity after:pointer-events-none after:absolute after:top-0 after:right-0 after:bottom-3 after:z-10 after:w-10 after:bg-gradient-to-l after:from-background after:to-transparent after:opacity-0 after:transition-opacity data-[left-fade=true]:before:opacity-100 data-[right-fade=true]:after:opacity-100 lg:-mx-4 lg:w-[calc(100%+2rem)] lg:before:w-[3.25rem] lg:after:w-[3.25rem]"
        data-benchmark-scroll-frame
      >
        <div
          ref={scrollRef}
          className="flex w-full snap-x snap-mandatory items-stretch gap-3 overflow-x-auto overscroll-x-contain scroll-px-2 px-2 pb-3 [scrollbar-width:none] lg:gap-4 lg:scroll-px-4 lg:px-4 [&::-webkit-scrollbar]:hidden"
        >
          {programs.map((program) => (
            <ProgramRow key={program.id} program={program} activeTab={activeTab} />
          ))}
        </div>
      </div>
    </div>
  )
}

function Stat({ value, label }: { value: string; label: string }) {
  return (
    <div className="border-border/80 min-w-0 rounded-2xl border-2 p-4">
      <div
        className={`text-foreground text-2xl leading-none font-medium sm:text-3xl ${tabularNums}`}
      >
        {value}
      </div>
      <div className="text-muted-foreground mt-2 text-[0.8125rem] leading-[100%]">{label}</div>
    </div>
  )
}

export default function PerformanceBenchmarks() {
  const [activeTab, setActiveTab] = useState<ActiveBenchmarkTab>('binary')
  const scrollRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const scrollArea = scrollRef.current
    if (!scrollArea) return

    scrollArea.scrollLeft = 0
    return bindHorizontalScrollFade(scrollArea, {
      frameSelector: '[data-benchmark-scroll-frame]',
      itemSelector: '[data-benchmark-row]',
    })
  }, [activeTab])

  return (
    <section
      id="performance-benchmarks"
      className="not-prose mt-28 scroll-mt-[calc(var(--docs-announcement-offset,0px)+5rem)] sm:mt-32"
      aria-labelledby="performance-benchmarks-title"
    >
      <div className="flex items-end justify-between gap-4">
        <div className="max-w-2xl">
          <h2
            id="performance-benchmarks-title"
            className="text-foreground m-0 text-2xl leading-[1.1] font-medium text-balance"
          >
            Anchor v2 in practice
          </h2>
          <p className="text-muted-foreground mt-2 mb-0 text-base leading-[1.45] text-pretty">
            Anchor v2 keeps the workflow developers rely on while making the generated program
            smaller, lighter, and cheaper to run.
          </p>
        </div>
      </div>

      <div className="mt-4 grid grid-cols-3 gap-2">
        <Stat value="95%" label="less deployed bytecode" />
        <Stat value="9.9x" label="average CU reduction" />
        <Stat value="50.4x" label="largest CU reduction" />
      </div>

      <div className="mt-8 flex w-full flex-col items-stretch sm:mt-10">
        <div className="flex w-full flex-wrap items-center justify-between gap-x-4 gap-y-3">
          <div
            role="tablist"
            aria-label="Benchmark metric"
            className="border-border/80 bg-muted/30 inline-flex h-[2.875rem] max-w-full items-center gap-1 overflow-x-auto rounded-md border-2 p-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
          >
            {benchmarkTabs.map((tab) => (
              <button
                key={tab.value}
                id={`benchmark-${tab.value}-tab`}
                type="button"
                role="tab"
                aria-selected={activeTab === tab.value}
                aria-controls={`benchmark-${tab.value}-panel`}
                className="text-muted-foreground hover:bg-muted/60 hover:text-foreground aria-selected:bg-background aria-selected:text-foreground h-[calc(2.875rem-0.5rem-4px)] flex-none cursor-pointer rounded-sm border-0 bg-transparent px-4 text-base leading-none font-medium whitespace-nowrap transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
                onClick={() => setActiveTab(tab.value)}
              >
                {tab.label}
              </button>
            ))}
          </div>

          <div
            className="border-border/80 bg-background text-muted-foreground inline-grid h-[2.875rem] content-center gap-1 rounded-md border-2 px-2.5 py-1.5 text-[0.6875rem] leading-none font-medium"
            aria-label="Benchmark comparison key"
          >
            <span className="inline-flex items-center gap-1.5 whitespace-nowrap">
              <span
                className={`${benchmarkBaselineColor} ${benchmarkBaselineColorDark} h-[0.3125rem] w-[1.125rem] flex-none rounded-full`}
                aria-hidden="true"
              />
              v1 baseline
            </span>
            <span className="inline-flex items-center gap-1.5 whitespace-nowrap">
              <span
                className="bg-accent h-[0.3125rem] w-[1.125rem] flex-none rounded-full"
                aria-hidden="true"
              />
              v2 runtime
            </span>
          </div>
        </div>

        <BenchmarkPanel activeTab={activeTab} scrollRef={scrollRef} />
      </div>
    </section>
  )
}
