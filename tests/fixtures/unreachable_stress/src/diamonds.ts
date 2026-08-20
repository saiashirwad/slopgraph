export function reachableDiamondA(): void {
  diamondJoin();
}

export function reachableDiamondB(): void {
  diamondJoin();
}

function diamondJoin(): void {
  deepChain1();
}

function deepChain1(): void {
  deepChain2();
}

function deepChain2(): void {}

export function deadBranch1(): void {
  deadBranch2();
}

function deadBranch2(): void {}
