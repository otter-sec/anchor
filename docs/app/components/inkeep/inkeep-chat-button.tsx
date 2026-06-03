"use client";

import {
  InkeepChatButton as ChatButton,
  InkeepChatButtonProps,
} from "@inkeep/cxkit-react";
import { useInkeepConfig } from "./useInkeepConfig";

export function InkeepChatButton() {
  const inkeepConfig = useInkeepConfig();
  return <ChatButton {...(inkeepConfig as InkeepChatButtonProps)} />;
}
