export function startPipeline(value: number): number {
  return forwardStep(value);
}

function forwardStep(value: number): number {
  return computeStep(value);
}

function computeStep(value: number): number {
  return value + 10;
}
