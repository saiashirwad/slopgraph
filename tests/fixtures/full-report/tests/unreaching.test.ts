import { unreachedProductionFn } from "../src/unreached_prod";

export function runUnreachingTest(): void {
  console.log("does not call unreachedProductionFn");
}
