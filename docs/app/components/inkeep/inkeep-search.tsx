"use client";

import type { SharedProps } from "fumadocs-ui/components/dialog/search";
import {
  InkeepModalSearchAndChat,
  type InkeepModalSearchAndChatProps,
} from "@inkeep/cxkit-react";
import { useInkeepConfig } from "./useInkeepConfig";

export default function CustomDialog(props: SharedProps) {
  const baseConfig = useInkeepConfig();
  const { open, onOpenChange } = props;

  const config = {
    ...baseConfig,
    modalSettings: {
      isOpen: open,
      onOpenChange,
    },
  };

  return (
    <InkeepModalSearchAndChat {...(config as InkeepModalSearchAndChatProps)} />
  );
}
