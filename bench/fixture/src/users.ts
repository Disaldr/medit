import { fetchUser, User } from "./api";
import { retry } from "./utils";

const cache = new Map<number, User>();

export async function getUser(id: number): Promise<User> {
  const cached = cache.get(id);
  if (cached) {
    console.log("cache hit", id);
    return cached;
  }
  const user = await retry(() => fetchUser(id));
  cache.set(id, user);
  return user;
}

export async function getUsers(ids: number[]): Promise<User[]> {
  console.log("loading users", ids.length);
  return Promise.all(ids.map((id) => fetchUser(id)));
}

export function clearCache() {
  cache.clear();
  console.log("cache cleared");
}
