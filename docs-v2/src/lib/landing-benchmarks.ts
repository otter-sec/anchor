export type BenchmarkMetric = {
  label: string
  detail: string
  olderLabel: string
  v2Label: string
  reductionLabel: string
  changeLabel: string
  v2Share: number
}

type InstructionMetric = BenchmarkMetric & {
  older: number
  v2: number
  reduction: number
}

export type ProgramBenchmark = {
  id: string
  label: string
  detail: string
  binary: BenchmarkMetric
  compute: BenchmarkMetric
}

export type ActiveBenchmarkTab = 'binary' | 'compute'

export const programs: ProgramBenchmark[] = [
  programBenchmark({
    id: 'helloworld',
    label: 'helloworld',
    detail: 'Single-instruction counter',
    binary: [124_800, 6_496],
    instructions: [{ label: 'init', detail: 'Counter initialization', older: 5_855, v2: 1_395 }],
  }),
  programBenchmark({
    id: 'prop-amm',
    label: 'prop-amm',
    detail: 'Oracle feed with asm fast-path',
    binary: [140_456, 8_344],
    instructions: [
      { label: 'initialize', detail: 'Oracle setup', older: 4_314, v2: 1_379 },
      { label: 'rotate_authority', detail: 'Authority rotation', older: 1_340, v2: 91 },
      { label: 'update', detail: 'Asm fast path', older: 1_310, v2: 26 },
    ],
  }),
  programBenchmark({
    id: 'vault',
    label: 'vault',
    detail: 'Single-depositor SOL vault',
    binary: [107_544, 5_424],
    instructions: [
      { label: 'deposit', detail: 'System transfer CPI', older: 5_707, v2: 1_905 },
      { label: 'withdraw', detail: 'Lamport withdrawal', older: 2_478, v2: 393 },
    ],
  }),
  programBenchmark({
    id: 'nested',
    label: 'nested',
    detail: 'Shared validation via Nested<T>',
    binary: [157_336, 13_264],
    instructions: [
      { label: 'initialize', detail: 'Shared validation setup', older: 19_842, v2: 2_872 },
      { label: 'increment', detail: 'Nested validation path', older: 4_751, v2: 477 },
      { label: 'reset', detail: 'Nested reset path', older: 4_752, v2: 476 },
    ],
  }),
  programBenchmark({
    id: 'multisig',
    label: 'multisig',
    detail: 'Four-instruction SOL multisig',
    binary: [170_096, 31_224],
    instructions: [
      { label: 'create', detail: 'Config creation', older: 12_031, v2: 3_039 },
      { label: 'deposit', detail: 'Vault funding', older: 4_872, v2: 1_627 },
      { label: 'set_label', detail: 'Inline PodVec update', older: 4_324, v2: 475 },
      { label: 'execute_transfer', detail: 'Threshold transfer', older: 7_446, v2: 2_182 },
    ],
  }),
]

export const benchmarkTabs = [
  { value: 'binary', label: 'Binary size' },
  { value: 'compute', label: 'Compute units' },
] satisfies Array<{ value: ActiveBenchmarkTab; label: string }>

function programBenchmark({
  id,
  label,
  detail,
  binary,
  instructions,
}: {
  id: string
  label: string
  detail: string
  binary: [older: number, v2: number]
  instructions: Array<{ label: string; detail: string; older: number; v2: number }>
}): ProgramBenchmark {
  const instructionMetrics = instructions.map((instruction) =>
    instructionMetric(instruction.label, instruction.detail, instruction.older, instruction.v2),
  )
  const leastImproved = instructionMetrics.reduce((current, next) =>
    next.v2Share > current.v2Share ? next : current,
  )

  return {
    id,
    label,
    detail,
    binary: singleMetric(label, detail, binary[0], binary[1], 'bytes', 'smaller'),
    compute: {
      label,
      detail,
      olderLabel: formatCuRange(instructionMetrics.map((instruction) => instruction.older)),
      v2Label: formatCuRange(instructionMetrics.map((instruction) => instruction.v2)),
      reductionLabel: formatReductionRange(
        instructionMetrics.map((instruction) => instruction.reduction),
      ),
      changeLabel: 'lower CU',
      v2Share: leastImproved.v2Share,
    },
  }
}

function singleMetric(
  label: string,
  detail: string,
  older: number,
  v2: number,
  unit: 'bytes' | 'cu',
  changeLabel: string,
): BenchmarkMetric {
  const reduction = older / v2

  return {
    label,
    detail,
    olderLabel: unit === 'bytes' ? formatKb(older) : `${formatNumber(older)} CU`,
    v2Label: unit === 'bytes' ? formatKb(v2) : `${formatNumber(v2)} CU`,
    reductionLabel: `${formatReduction(reduction)}x`,
    changeLabel,
    v2Share: (v2 / older) * 100,
  }
}

function instructionMetric(
  label: string,
  detail: string,
  older: number,
  v2: number,
): InstructionMetric {
  const reduction = older / v2

  return {
    label,
    detail,
    older,
    v2,
    olderLabel: `${formatNumber(older)} CU`,
    v2Label: `${formatNumber(v2)} CU`,
    reduction,
    reductionLabel: `${formatReduction(reduction)}x`,
    changeLabel: 'lower CU',
    v2Share: (v2 / older) * 100,
  }
}

function formatKb(bytes: number): string {
  const kb = bytes / 1000
  return `${kb >= 10 ? kb.toFixed(1).replace('.0', '') : kb.toFixed(1)} KB`
}

function formatNumber(value: number): string {
  return new Intl.NumberFormat('en-US').format(value)
}

function formatReduction(value: number): string {
  return value >= 10 ? value.toFixed(1).replace('.0', '') : value.toFixed(1)
}

function formatCuRange(values: number[]): string {
  const min = Math.min(...values)
  const max = Math.max(...values)

  if (min === max) return `${formatNumber(min)} CU`

  return `${formatNumber(min)}-${formatNumber(max)} CU`
}

function formatReductionRange(values: number[]): string {
  const min = Math.min(...values)
  const max = Math.max(...values)

  if (min === max) return `${formatReduction(min)}x`

  return `${formatReduction(min)}-${formatReduction(max)}x`
}
