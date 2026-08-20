export function orphanFn1(): void {
  orphanFn2();
}

function orphanFn2(): void {}
