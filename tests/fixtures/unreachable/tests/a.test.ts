import { testOnly } from "../src/test_only";
import { usedHelper, testOnlyHelper } from "../src/used";

export function testA(): number {
  const a = 1;
  usedHelper();
  testOnlyHelper();
  testOnly();
  return a;
}
