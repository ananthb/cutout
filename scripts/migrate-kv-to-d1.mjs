#!/usr/bin/env node
import { execFileSync } from "node:child_process";

try {
  const countRaw = execFileSync(
    "npx",
    [
      "wrangler",
      "d1",
      "execute",
      "cutout-db",
      "--remote",
      "--json",
      "--command",
      "SELECT COUNT(*) AS count FROM rules_state WHERE id = 1",
    ],
    { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }
  );

  const countParsed = JSON.parse(countRaw);
  const count =
    Number(countParsed?.[0]?.results?.[0]?.count ?? 0);

  if (count > 0) {
    console.log("D1 rules_state already populated; skipping KV migration");
    process.exit(0);
  }

  const nsListRaw = execFileSync(
    "npx",
    ["wrangler", "kv", "namespace", "list"],
    { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }
  );

  const namespaces = JSON.parse(nsListRaw);
  if (!Array.isArray(namespaces) || namespaces.length === 0) {
    console.log("No KV namespaces found; skipping KV migration");
    process.exit(0);
  }

  namespaces.sort((a, b) => {
    const score = (ns) => {
      const title = String(ns?.title ?? "").toLowerCase();
      if (title.includes("cutout")) return 2;
      if (title.includes("kv")) return 1;
      return 0;
    };
    return score(b) - score(a);
  });

  for (const ns of namespaces) {
    if (!ns?.id) continue;
    let rawValue;
    try {
      rawValue = execFileSync(
        "npx",
        [
          "wrangler",
          "kv",
          "key",
          "get",
          "--namespace-id",
          String(ns.id),
          "--remote",
          "rules",
          "--text",
        ],
        { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }
      );
    } catch {
      continue;
    }

    if (!rawValue || !rawValue.trim()) continue;

    let parsed;
    try {
      parsed = JSON.parse(rawValue);
    } catch {
      continue;
    }

    let version = 1;
    let rulesJson;
    if (Array.isArray(parsed)) {
      version = 1;
      rulesJson = JSON.stringify(parsed);
    } else if (parsed && typeof parsed === "object" && Array.isArray(parsed.rules)) {
      version = Math.max(1, Number(parsed.version) || 1);
      rulesJson = JSON.stringify(parsed.rules);
    } else {
      continue;
    }

    const escapedJson = rulesJson.replace(/'/g, "''");
    execFileSync(
      "npx",
      [
        "wrangler",
        "d1",
        "execute",
        "cutout-db",
        "--remote",
        "--command",
        `INSERT OR IGNORE INTO rules_state (id, version, rules_json) VALUES (1, ${version}, '${escapedJson}')`,
      ],
      { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }
    );

    console.log(
      `Migrated rules (version ${version}) from KV namespace ${ns.id} to D1`
    );
    process.exit(0);
  }

  console.log("No rules key found in KV namespaces; skipping KV migration");
  process.exit(0);
} catch (err) {
  console.log("KV migration skipped:", err?.message ?? String(err));
  process.exit(0);
}
