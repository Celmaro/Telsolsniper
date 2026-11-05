// Transplant the authenticated Telethon auth key from the old radar session
// into a Teleproto StringSession for Telsolsniper's listener.
// Reads:  ~/tg-crypto-radar/radar.session (Telethon SQLite session, read-only)
// Writes: data/telegram.session (Teleproto StringSession, mode 600)
import { DatabaseSync } from "node:sqlite";
import { StringSession } from "teleproto/sessions";
import { TelegramClient } from "teleproto";
import { mkdir, writeFile, chmod } from "node:fs/promises";

const SRC = "/home/agentuser/tg-crypto-radar/radar.session";
const DST = "./data/telegram.session";
const apiId = Number(process.env.TELEGRAM_API_ID);
const apiHash = process.env.TELEGRAM_API_HASH;

const db = new DatabaseSync(SRC, { readOnly: true });
const row = db.prepare("SELECT dc_id, server_address, port, auth_key FROM sessions").get();
if (!row?.auth_key) throw new Error("source session has no auth_key");
const authKey = Buffer.from(row.auth_key);
console.log("source dc:", row.dc_id, "addr:", row.server_address, "port:", row.port, "key:", authKey.length, "bytes");

// Build the session string in teleproto's exact v1 wire format:
// "1" + base64(dcId(1) + addrLen(2) + addr + port(2) + authKey(256))
import { Buffer as NBuffer } from "node:buffer";
const dcBuffer = NBuffer.from([row.dc_id]);
const addressBuffer = NBuffer.from(row.server_address, "utf8");
const addressLengthBuffer = NBuffer.alloc(2);
addressLengthBuffer.writeUInt16BE(addressBuffer.length, 0);
const portBuffer = NBuffer.alloc(2);
portBuffer.writeUInt16BE(row.port, 0);
const payload = NBuffer.concat([dcBuffer, addressLengthBuffer, addressBuffer, portBuffer, authKey]);
const encoded = "1" + payload.toString("base64");
console.log("encoded session length:", encoded.length);

// Validate the built session actually authorizes before writing it to disk.
const client = new TelegramClient(new StringSession(encoded), apiId, apiHash, {
  connectionRetries: 3,
  useWSS: false,
});
await client.connect();
const me = await client.getMe();
console.log("authorized as:", me.username ?? me.firstName ?? String(me.id));
await client.disconnect();

await mkdir("./data", { recursive: true });
await writeFile(DST, encoded, { mode: 0o600 });
await chmod(DST, 0o600);
console.log("session written to", DST);
