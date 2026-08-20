export function sharedService(): void {
  console.log("service");
}

export function testOnlyService(): void {
  console.log("test only");
}

export function deadServiceHelper(): void {
  console.log("unreachable helper");
}
