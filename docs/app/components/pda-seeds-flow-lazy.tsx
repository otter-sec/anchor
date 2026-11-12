"use client";

import dynamic from "next/dynamic";

// ssr: false (client modules only) keeps @solana/* off the other docs pages.
export const PdaSeedsFlow = dynamic(
  () => import("./pda-seeds-flow").then(m => m.PdaSeedsFlow),
  {
    ssr: false,
    loading: () => <div className="h-[1690px] sm:h-[1100px]" />,
  },
);
