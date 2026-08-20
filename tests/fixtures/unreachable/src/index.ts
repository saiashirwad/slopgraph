import { usedHelper } from "./used";

export function rootIndex(): number {
  const x = 1;
  usedHelper();
  return x;
}
