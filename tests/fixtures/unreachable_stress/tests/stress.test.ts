import { testReachableHelper, testReachableChainA } from "../src/test_reachability";

export function testMain(): void {
  testReachableHelper();
  testReachableChainA();
}

function deadTestFn(): void {}
