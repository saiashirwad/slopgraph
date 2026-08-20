import { calculateTotal } from "../src/reached";

export function testCalculate(): void {
  const total = calculateTotal(10, 20);
  console.log(total);
}
