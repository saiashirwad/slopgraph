export interface Order {
  id: number;
}

export function handleOrder(order: Order): void {
  prepareOrder(order);
}

function prepareOrder(order: Order): void {
  validateAndSave(order);
}

function validateAndSave(order: Order): void {
  dbSave(order);
}

function dbSave(order: Order): void {
  console.log(order);
}

function persistOrder(order: Order): void {
  saveOrder(order);
}

function saveOrder(order: Order): void {
  console.log(order);
}
