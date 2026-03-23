import {
  chmodSync,
  existsSync,
  readFileSync,
  unlinkSync,
  writeFileSync,
} from "fs";
import { dirname, join } from "path";
import { execSync } from "child_process";
import { fileURLToPath } from "url";

const TARGETS = {
  "darwin-arm64": "aarch64-apple-darwin",
  "darwin-x64": "x86_64-apple-darwin",
  "linux-x64": "x86_64-unknown-linux-gnu",
  "linux-arm64": "aarch64-unknown-linux-gnu",
  "win32-x64": "x86_64-pc-windows-msvc",
  "win32-arm64": "aarch64-pc-windows-msvc",
};

const __dirname = dirname(fileURLToPath(import.meta.url));
const isWindows = process.platform === "win32";
const binaryPath = join(__dirname, isWindows ? "apitally.exe" : "apitally");

if (existsSync(binaryPath)) {
  process.exit(0);
}

const key = `${process.platform}-${process.arch}`;
const target = TARGETS[key];
if (!target) {
  console.error(`Unsupported platform: ${process.platform} ${process.arch}`);
  process.exit(1);
}

const { version } = JSON.parse(
  readFileSync(join(__dirname, "package.json"), "utf8"),
);
const archiveExt = isWindows ? ".zip" : ".tar.gz";
const url = `https://github.com/apitally/cli/releases/download/v${version}/apitally-${target}${archiveExt}`;
const archivePath = join(__dirname, `apitally${archiveExt}`);

try {
  console.log(`Downloading apitally binary for ${target}...`);
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  writeFileSync(archivePath, Buffer.from(await res.arrayBuffer()));

  if (isWindows) {
    execSync(
      `powershell -Command "Expand-Archive -Force -Path '${archivePath}' -DestinationPath '${__dirname}'"`,
      { stdio: "ignore" },
    );
  } else {
    execSync(`tar xzf "${archivePath}" -C "${__dirname}"`, {
      stdio: "ignore",
    });
    chmodSync(binaryPath, 0o755);
  }

  unlinkSync(archivePath);
  console.log("Done.");
} catch (err) {
  console.error(`Failed to install apitally binary: ${err.message}`);
  process.exit(1);
}
