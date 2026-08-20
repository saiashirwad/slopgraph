// 1. Happy path: Interface vs Interface (>= 3 fields)
export interface UserDTO {
  id: string;
  name: string;
  email: string;
}

export interface UserRecord {
  id: string;
  name: string;
  email: string;
}

// 2. Type Alias vs Interface (>= 3 fields)
export interface OrderInfo {
  orderId: string;
  total: number;
  status: string;
}

export type OrderPayload = {
  orderId: string;
  total: number;
  status: string;
};

// 3. Type Alias vs Type Alias (>= 3 fields)
export type ItemData = {
  sku: string;
  price: number;
  quantity: number;
};

export type ProductEntry = {
  sku: string;
  price: number;
  quantity: number;
};

// 4. Field permutation (order-independent matching)
export interface Point3DA {
  x: number;
  y: number;
  z: number;
}

export interface Point3DB {
  z: number;
  x: number;
  y: number;
}

// 5. Cross-file clone target (matches ServiceConfig in cross_file.ts)
export interface ServerSettings {
  host: string;
  port: number;
  secure: boolean;
}

// Negative 1: Only 2 fields (< 3 fields)
export interface PairA {
  first: string;
  second: number;
}

export interface PairB {
  first: string;
  second: number;
}

// Negative 2: Only 1 field (< 3 fields)
export interface SingleA {
  value: string;
}

export interface SingleB {
  value: string;
}

// Negative 3: Different field types (count: number vs count: string)
export interface MetricA {
  id: string;
  count: number;
  valid: boolean;
}

export interface MetricB {
  id: string;
  count: string;
  valid: boolean;
}

// Negative 4: Different field names (r,g,b vs c,m,y)
export interface ColorRGB {
  r: number;
  g: number;
  b: number;
}

export interface ColorCMY {
  c: number;
  m: number;
  y: number;
}

// Negative 5: Heritage clause (extends relationship)
export interface BaseEntity {
  id: string;
  createdAt: number;
  updatedAt: number;
}

export interface ExtendedEntity extends BaseEntity {
  id: string;
  createdAt: number;
  updatedAt: number;
}

// Negative 6: Optionality mismatch (name?: string vs name: string)
export interface OptA {
  id: string;
  name?: string;
  age: number;
}

export interface OptB {
  id: string;
  name: string;
  age: number;
}
