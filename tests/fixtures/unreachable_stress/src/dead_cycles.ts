export function deadCycle1(): void {
  deadCycle2();
}

export function deadCycle2(): void {
  deadCycle3();
}

export function deadCycle3(): void {
  deadCycle1();
}

export function selfRecursiveDead(): void {
  selfRecursiveDead();
}
