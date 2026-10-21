import { fetchOrders, fetchUser } from "./api";
import { config } from "./config";

export async function getOrderSummary(userId: number) {
  const user = await fetchUser(userId);
  const orders = await fetchOrders(userId);
  console.log("orders loaded", orders.length);
  return {
    user: user.name,
    total: orders.length,
    pages: Math.ceil(orders.length / config.pageSize),
  };
}
