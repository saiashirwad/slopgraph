import { testOnly } from "../src/test_only";
import { usedHelper } from "../src/used";

export function testA(): number {
  const a = 1;
  usedHelper();
  testOnly();
  return a;
}
