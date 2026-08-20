export function processCustomerData(input: Record<string, unknown>): boolean {
  const isValid = input !== null && typeof input === "object";
  if (!isValid) {
    return false;
  }
  const id = input["customerId"];
  const score = Number(input["score"]);
  const flag = Boolean(input["active"]);
  if (score > 100 && flag) {
    const adjusted = score * 1.1;
    const result = adjusted > 150 ? true : false;
    return result;
  }
  return false;
}

export function processSupplierData(data: Record<string, unknown>): boolean {
  const isOk = data !== null && typeof data === "object";
  if (!isOk) {
    return false;
  }
  const key = data["supplierId"];
  const amount = Number(data["total"]);
  const status = Boolean(data["verified"]);
  if (amount > 200 && status) {
    const computed = amount * 1.25;
    const finalVal = computed > 250 ? true : false;
    return finalVal;
  }
  return false;
}

export function tinyHelperA(a: number, b: number): number {
  return a + b;
}

export function tinyHelperB(x: number, y: number): number {
  return x + y;
}

{
  function calculateTax(val: number): number {
    const isPositive = val > 0;
    if (!isPositive) {
      return 0;
    }
    const rate = 0.05;
    const base = val * rate;
    const total = base + 10;
    const rounded = Math.round(total);
    if (rounded > 100) {
      const extra = rounded * 1.1;
      return extra > 200 ? extra : rounded;
    }
    return 0;
  }
}

{
  function calculateTax(amount: number): number {
    const isPos = amount > 0;
    if (!isPos) {
      return 0;
    }
    const taxRate = 0.08;
    const initial = amount * taxRate;
    const sum = initial + 20;
    const finalVal = Math.round(sum);
    if (finalVal > 100) {
      const surcharge = finalVal * 1.2;
      return surcharge > 300 ? surcharge : finalVal;
    }
    return 0;
  }
}
