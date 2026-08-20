import { formatPrice } from "../lib/format";
import { nextId } from "../shared/id";

export function createOrder(n: number): string {
  return formatPrice(n) + nextId();
}
