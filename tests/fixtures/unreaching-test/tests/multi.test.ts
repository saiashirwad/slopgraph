import { formatGreeting } from "../src/multi_reached";
import { unusedFeature } from "../src/multi_unreached";

export function testMulti(): void {
  const str = formatGreeting("world");
  console.log(str);
}
