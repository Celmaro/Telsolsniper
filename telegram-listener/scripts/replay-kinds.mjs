import { readFileSync } from "node:fs";
import { parseTelegramSignals, classifySignal } from "../src/parser.ts";
const CH = ["soltrenchtrending","solwhaletrending","solearlytrending"];
let stats = {};
for (const ch of CH) {
  const rows = readFileSync(`data/captures/${ch}.jsonl`,"utf8").trim().split("\n").map(JSON.parse);
  for (const r of rows) {
    const urls = [...(r.surfaces?.entityUrls??[]), ...(r.surfaces?.hiddenUrls??[]), ...(r.surfaces?.buttonUrls??[])];
    const parsed = parseTelegramSignals(ch, r.messageId, r.text ?? "", r.date, urls);
    if (!parsed.length) continue;
    const kind = parsed[0].kind;
    stats[ch] ??= { total:0, byKind:{}, multi:0, disagreement:0 };
    stats[ch].total++;
    stats[ch].byKind[kind] = (stats[ch].byKind[kind]??0)+1;
    const raw = [...new Set(r.mints?.fromUrls ?? [])];
    if (raw.length > 1) {
      stats[ch].multi++;
      if (raw[0] !== parsed[0].mint) stats[ch].disagreement++;
    }
  }
}
for (const [ch, s] of Object.entries(stats)) {
  console.log(`\n${ch}: posts_with_mints=${s.total} multi_mint=${s.multi} order_fixed=${s.disagreement}`);
  console.log("  kinds:", s.byKind);
}
