import { testOnlyService } from "../src/service";

export function runFullTest(): void {
  console.log("test running");
  testOnlyService();
}
