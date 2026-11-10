import { readFileSync } from "node:fs";
import { classifySignal } from "../src/parser.ts";
for (const ch of ["soltrenchtrending","solwhaletrending","solearlytrending"]) {
  const rows = readFileSync(`data/captures/${ch}.jsonl`,"utf8").trim().split("\n").map(JSON.parse);
  const seen = new Set();
  console.log(`\n===== ${ch} =====`);
  for (const kind of ["entry","followup","victory_lap","leaderboard","other"]) {
    const samples = rows.filter(r => classifySignal(r.text ?? "")===kind).slice(0,2);
    for (const r of samples) {
      const line = (r.text??"").split("\n").slice(0,2).join(" | ").slice(0,95);
      console.log(`  [${kind}] ${line}`);
    }
  }
}
