export function entryRoot(x: number): number {
  return exportedMiddle(x);
}

export function exportedMiddle(x: number): number {
  return finalLeaf(x);
}

function finalLeaf(x: number): number {
  return x * 2;
}
