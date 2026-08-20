export function testReachableHelper(): number {
  return 123;
}

export function testReachableChainA(): void {
  testReachableChainB();
}

function testReachableChainB(): void {}

export function completelyDeadInTestReachableMod(): void {}
