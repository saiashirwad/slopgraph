import { unreachedProductionFn } from "../src/unreached";
import { setupTestEnvironment } from "./helper";

export function testUnreached(): void {
  const ok = setupTestEnvironment();
  console.log(ok);
}
