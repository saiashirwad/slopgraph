export function sink(val: unknown): void {}

export function forwardSimple(data: string): void {
  sink(data);
}

export function forwardWithAssert(info: any): void {
  sink((info as string)!);
}

export function multiHopA(item: string): void {
  multiHopB(item);
}

function multiHopB(item: string): void {
  sink(item);
}

export function readProperty(param: { val: string }): void {
  console.log(param.val);
  sink(param);
}

export function checkCondition(param: string): void {
  if (param) {
    sink(param);
  }
}

export function untypedCallOnly(param: string): void {
  console.log(param);
}

export function unusedParam(param: string): void {
  sink("literal");
}

export function operatorRead(param: number): void {
  sink(param + 1);
}

export function calleeParam(param: () => void): void {
  param();
}
