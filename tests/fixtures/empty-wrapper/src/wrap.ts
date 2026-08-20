function saveOrder(order: number): number {
  return order + 1;
}

function persistOrder(order: number): number {
  return saveOrder(order);
}

function asPersist(order: number): number {
  return saveOrder(order) as number;
}

function fireAndForget(order: number): void {
  saveOrder(order);
}

export function exportedWrap(order: number): number {
  return saveOrder(order);
}

function unresolved(f: any): void {
  f(1);
}

function notAWrapper(order: number): number {
  const x = saveOrder(order);
  return x;
}
