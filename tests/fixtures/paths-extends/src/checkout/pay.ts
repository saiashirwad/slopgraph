import { formatMoney } from "@lib/money";

export function pay(n: number): string {
  return formatMoney(n);
}
