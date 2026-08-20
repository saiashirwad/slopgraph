export function productionCaller(x: number): number {
  return helperWithTest(x);
}

export function helperWithTest(x: number): number {
  return x + 1;
}
