#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ext = process.platform === "win32" ? ".exe" : "";
const binaryPath = join(__dirname, `apitally${ext}`);

if (!existsSync(binaryPath)) {
  console.error(
    "apitally binary not found. Try reinstalling: npm install -g @apitally/cli",
  );
  process.exit(1);
}

try {
  execFileSync(binaryPath, process.argv.slice(2), { stdio: "inherit" });
} catch (e) {
  process.exit(e.status ?? 1);
}
