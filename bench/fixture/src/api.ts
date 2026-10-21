import { config } from "./config";

export interface User {
  id: number;
  name: string;
  email: string;
}

export async function fetchUser(id: number): Promise<User> {
  console.log("fetching user", id);
  const res = await fetch(`${config.baseUrl}/users/${id}`, {
    signal: AbortSignal.timeout(config.timeout),
  });
  if (!res.ok) {
    console.log("request failed", res.status);
    throw new Error(`HTTP ${res.status}`);
  }
  return res.json();
}

export async function fetchOrders(userId: number) {
  console.log("fetching orders", userId);
  const res = await fetch(`${config.baseUrl}/users/${userId}/orders`);
  return res.json();
}
