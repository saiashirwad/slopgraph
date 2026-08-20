export function reachableA(): void {
  cycleNode1();
}

export function cycleNode1(): void {
  cycleNode2();
}

export function cycleNode2(): void {
  // Reachable cycle back to cycleNode1
  cycleNode1();
}
