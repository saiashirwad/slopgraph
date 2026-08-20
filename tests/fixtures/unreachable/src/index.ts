import { usedHelper } from "./used";

function unusedIndexFn(): void {}

export function rootIndex(): number {
  const x = 1;
  usedHelper();
  return x;
}
