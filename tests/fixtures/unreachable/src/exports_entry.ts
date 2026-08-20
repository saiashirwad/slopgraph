import { usedHelper } from "./used";

export function exportsEntry(): number {
  const w = 4;
  usedHelper();
  return w;
}
