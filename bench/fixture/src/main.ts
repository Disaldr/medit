import { getUser, getUsers } from "./users";
import { getOrderSummary } from "./orders";
import { fetchUser } from "./api";

async function main() {
  console.log("start");
  const me = await getUser(1);
  console.log("me", me.name);
  const team = await getUsers([2, 3, 4]);
  console.log("team size", team.length);
  const raw = await fetchUser(5);
  console.log("raw", raw.id);
  const summary = await getOrderSummary(1);
  console.log("summary", summary);
}

main();
