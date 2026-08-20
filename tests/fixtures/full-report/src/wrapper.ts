export function targetAction(): string {
  return "target";
}

export function emptyWrapper(): string {
  return targetAction();
}

export function wrapFirst(): void {
  console.log("first");
  emptyWrapper();
}

export function wrapSecond(): void {
  console.log("second");
  emptyWrapper();
}
