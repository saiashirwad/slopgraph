import { reachableA } from "./reachable_cycles";
import { deadCycle1 } from "./dead_cycles";
import { reachableDiamondA, reachableDiamondB, deadBranch1 } from "./diamonds";
import { completelyDeadInTestReachableMod } from "./test_reachability";

function privateDeadInEntry(): void {}

export function entryFn(): void {
  reachableA();
  reachableDiamondA();
  reachableDiamondB();
}
