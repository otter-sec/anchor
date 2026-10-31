"use client";

import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
  type Ref,
} from "react";
import {
  address,
  getAddressEncoder,
  getAddressFromPublicKey,
  getProgramDerivedAddress,
  type Address,
} from "@solana/addresses";
import { generateKeyPair } from "@solana/keys";

type Derived = { pda: Address; bump: number } | null;
type Kind = "program" | "seed" | "signer";
type Link = [x1: number, y1: number, x2: number, y2: number];

const kindColors = {
  TEXT: "bg-[#5b1f33]",
  KEYPAIR: "bg-[#1e5f74]",
  PDA: "bg-[#5b2a6e]",
};

const labels: Record<Kind, string> = {
  program: "program id",
  seed: "static seed",
  signer: "signer",
};

const programId = "BZLiJ62bzRryYp9mRobz47uA66WDgtfTXhhgM25tJyx5";

const examples: (BlockProps & { title: string; text: string })[] = [
  {
    title: "seeds = []",
    text: "No seeds: the PDA depends only on the program id.",
    inputs: ["program"],
  },
  {
    title: 'seeds = [b"hello_world"]',
    text: "Change the program id or the seed and the PDA changes.",
    inputs: ["program", "seed"],
  },
  {
    title: 'seeds = [b"hello_world", signer.key().as_ref()]',
    text: "Press Generate: a new signer gives a new PDA, the other blocks don't change.",
    inputs: ["program", "seed", "signer"],
  },
  {
    title: 'seeds = [b"hello_world"], seeds::program = other_program.key()',
    text: "The PDA is derived from other_program, so changing it changes the address even with the same seeds.",
    inputs: ["program", "seed"],
    program: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
    programLabel: "other program id",
  },
];

async function derive(
  [program, ...seeds]: Kind[],
  values: Record<Kind, string>,
): Promise<Derived> {
  try {
    const [pda, bump] = await getProgramDerivedAddress({
      programAddress: address(values[program]),
      seeds: seeds.map(kind =>
        kind === "signer"
          ? getAddressEncoder().encode(address(values.signer))
          : values[kind],
      ),
    });
    return { pda, bump };
  } catch {
    return null;
  }
}

type CardProps = {
  kind: keyof typeof kindColors;
  children: ReactNode;
  bodyRef?: Ref<HTMLDivElement>;
};

function Card({ kind, children, bodyRef }: CardProps) {
  return (
    <div className="rounded-md border bg-fd-background text-xs shadow-sm">
      <div ref={bodyRef} className="space-y-1.5 p-2">
        {children}
      </div>
      <div
        className={`rounded-b-md py-0.5 text-center text-[11px] font-semibold tracking-wider text-white ${kindColors[kind]}`}
      >
        {kind}
      </div>
    </div>
  );
}

type BlockProps = { inputs: Kind[]; program?: string; programLabel?: string };

function Block({
  inputs,
  program = programId,
  programLabel = labels.program,
}: BlockProps) {
  const [values, setValues] = useState({
    program,
    seed: "hello_world",
    signer: "",
  });
  const [keygenFailed, setKeygenFailed] = useState(false);
  const [result, setResult] = useState<Derived>();
  const [links, setLinks] = useState<Link[]>([]);
  const svgRef = useRef<SVGSVGElement>(null);
  const pdaRef = useRef<HTMLDivElement>(null);
  const pdaBodyRef = useRef<HTMLDivElement>(null);
  const inputRefs = useRef<(HTMLDivElement | null)[]>([]);
  const blockLabels = { ...labels, program: programLabel };

  const generate = useCallback(() => {
    generateKeyPair()
      .then(({ publicKey }) => getAddressFromPublicKey(publicKey))
      .then(
        signer => setValues(v => ({ ...v, signer })),
        () => setKeygenFailed(true),
      );
  }, []);

  useEffect(() => {
    if (inputs.includes("signer")) generate();
  }, [inputs, generate]);

  useEffect(() => {
    if (inputs.includes("signer") && !values.signer) return;
    let stale = false;
    derive(inputs, values).then(derived => {
      if (!stale) setResult(derived);
    });
    return () => {
      stale = true;
    };
  }, [inputs, values]);

  useLayoutEffect(() => {
    const svg = svgRef.current;
    const pda = pdaRef.current;
    const body = pdaBodyRef.current;
    if (!svg || !pda || !body) return;
    const update = () => {
      const box = svg.getBoundingClientRect();
      const x2 = pda.getBoundingClientRect().left - box.left;
      const target = body.getBoundingClientRect();
      const sources = inputRefs.current.flatMap(el =>
        el ? [el.getBoundingClientRect()] : [],
      );
      const next = sources.map(
        (source, i): Link => [
          source.right - box.left,
          source.top + source.height / 2 - box.top,
          x2,
          target.top +
            (target.height * (i + 1)) / (sources.length + 1) -
            box.top,
        ],
      );
      setLinks(prev => (String(prev) === String(next) ? prev : next));
    };
    const observer = new ResizeObserver(update);
    observer.observe(svg);
    observer.observe(body);
    return () => observer.disconnect();
  }, []);

  const signerText = keygenFailed
    ? "Ed25519 is not supported in this browser"
    : values.signer || "…";
  const pdaText = keygenFailed
    ? "needs a signer"
    : result === null
      ? "invalid input"
      : (result?.pda ?? "…");

  return (
    <div className="relative grid gap-3 rounded-lg border bg-fd-card p-2 sm:grid-cols-[3fr_2fr] sm:gap-x-12">
      <svg
        ref={svgRef}
        aria-hidden
        className="pointer-events-none absolute inset-0 hidden size-full text-fd-muted-foreground sm:block"
      >
        {links.map(([x1, y1, x2, y2], i) => {
          const mid = (x1 + x2) / 2;
          return (
            <g key={i} fill="currentColor">
              <path
                d={`M${x1},${y1} C${mid},${y1} ${mid},${y2} ${x2},${y2}`}
                fill="none"
                stroke="currentColor"
              />
              <circle cx={x1} cy={y1} r={3} />
              <circle cx={x2} cy={y2} r={3} />
            </g>
          );
        })}
      </svg>
      <div className="space-y-3">
        {inputs.map((kind, i) => (
          <div
            key={kind}
            ref={el => {
              inputRefs.current[i] = el;
            }}
          >
            <Card kind={kind === "signer" ? "KEYPAIR" : "TEXT"}>
              <div className="flex items-center justify-between text-fd-muted-foreground">
                {blockLabels[kind]}
                {kind === "signer" && (
                  <button
                    type="button"
                    className="rounded border bg-fd-card px-2 py-0.5 font-medium text-fd-foreground hover:bg-fd-accent"
                    onClick={generate}
                  >
                    Generate
                  </button>
                )}
              </div>
              <input
                aria-label={blockLabels[kind]}
                readOnly={kind === "signer"}
                className="w-full rounded border bg-fd-card px-1 py-1 font-mono text-[11px]"
                value={kind === "signer" ? signerText : values[kind]}
                onChange={e =>
                  setValues(v => ({ ...v, [kind]: e.target.value }))
                }
              />
            </Card>
          </div>
        ))}
      </div>
      <div
        aria-hidden
        className="text-center text-fd-muted-foreground sm:hidden"
      >
        ↓
      </div>
      <div ref={pdaRef} className="self-center">
        <Card kind="PDA" bodyRef={pdaBodyRef}>
          <div className="break-all font-mono text-[11px]">{pdaText}</div>
          <div className="font-mono text-[11px] text-fd-muted-foreground">
            bump {result?.bump ?? "–"}
          </div>
        </Card>
      </div>
    </div>
  );
}

export function PdaSeedsFlow() {
  return (
    <div className="not-prose space-y-6">
      {examples.map(({ title, text, ...block }) => (
        <section key={title} aria-label={title} className="space-y-2">
          <div>
            <code className="block text-sm font-semibold">{title}</code>
            <p className="text-sm text-fd-muted-foreground">{text}</p>
          </div>
          <Block {...block} />
        </section>
      ))}
    </div>
  );
}
