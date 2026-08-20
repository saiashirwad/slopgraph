export function sink(val: unknown): void {
  console.log(val);
}

export function forwardData(payload: string): void {
  console.log("forwarding");
  sink(payload);
}
