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
