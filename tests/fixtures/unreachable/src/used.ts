export function usedHelper(): number {
  return 10;
}

export function deadHelper(): number {
  return 99;
}

function deadInternal(): void {}

export function deadChainA(): void {
  deadChainB();
}

function deadChainB(): void {}

export function testOnlyHelper(): number {
  return 42;
}
