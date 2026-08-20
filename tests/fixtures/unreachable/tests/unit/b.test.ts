import { testOnly } from "../../src/test_only";

export function testB(): number {
  const b = 2;
  testOnly();
  return b;
}
