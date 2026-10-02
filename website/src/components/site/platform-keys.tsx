"use client";

import { Keycaps, shortcut } from "../neptune/controls";
import { useMac } from "../prefs";

/**
 * A primary shortcut, shown the way the visitor's platform writes it. An
 * empty name shows the modifiers alone.
 */
export function PlatformKeys({ name }: { name: string }) {
  const mac = useMac();
  return <Keycaps chord={name ? shortcut(mac, name) : mac ? "⌘" : "Ctrl+Shift"} />;
}
