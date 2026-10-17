import Script from "next/script";
import type { DetailedHTMLProps, HTMLAttributes } from "react";

declare module "react" {
  namespace JSX {
    interface IntrinsicElements {
      "plgrnd-flow": DetailedHTMLProps<
        HTMLAttributes<HTMLElement>,
        HTMLElement
      > & {
        flow: string;
        theme?: "light" | "dark" | "auto";
        "link-ref"?: string;
      };
    }
  }
}

export function PlgrndFlow({ flow }: { flow: string }) {
  return (
    <>
      <Script
        src="/plgrnd/plgrnd-flow-1.0.1.js"
        integrity="sha384-Y2BkqcJKLd+pVOsJ4EU/v0QoThcL8XpjFHeKoy7sLj24U73lzYK2VnShhVe7qv3B"
        strategy="lazyOnload"
      />
      <plgrnd-flow
        flow={flow}
        theme="auto"
        link-ref="anchor-docs"
        style={{ display: "block", height: 480, borderRadius: 8 }}
      />
    </>
  );
}
