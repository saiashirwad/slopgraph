import { runPipeline } from "./pipeline";
import { processCustomerData, processSupplierData } from "./duplicates";
import { forwardData } from "./tramp";
import { wrapFirst, wrapSecond } from "./wrapper";
import { sharedService } from "./service";
import { UserProfile, AccountRecord } from "./types";

export function main(): void {
  runPipeline();
  processCustomerData({ customerId: "1", score: 120, active: true });
  processSupplierData({ supplierId: "2", total: 250, verified: true });
  forwardData("payload1");
  forwardData("payload2");
  wrapFirst();
  wrapSecond();
  sharedService();
  const u: UserProfile = { id: "1", name: "Alice", email: "a@example.com" };
  const a: AccountRecord = { id: "1", name: "Alice", email: "a@example.com" };
  console.log(u, a);
}
