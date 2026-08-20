import { queryData } from "./repo";

export function serviceAction(): string {
  const data = queryData();
  return `processed: ${data}`;
}
