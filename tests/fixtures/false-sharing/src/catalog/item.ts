import { nextId } from "../shared/id";

export function item(): string {
  return nextId();
}
