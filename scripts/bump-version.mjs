import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageJsonPath = path.join(repoRoot, "package.json");
const packageLockPath = path.join(repoRoot, "package-lock.json");
const tauriConfigPath = path.join(repoRoot, "src-tauri", "tauri.conf.json");
const cargoTomlPath = path.join(repoRoot, "src-tauri", "Cargo.toml");
const cargoLockPath = path.join(repoRoot, "src-tauri", "Cargo.lock");

function read(filePath) {
  return fs.readFileSync(filePath, "utf8");
}

function write(filePath, contents) {
  fs.writeFileSync(filePath, contents, "utf8");
}

function bumpPatch(version) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
  if (!match) {
    throw new Error(`La versión debe ser SemVer estable (X.Y.Z), recibida: ${version}`);
  }
  return `${match[1]}.${match[2]}.${Number(match[3]) + 1}`;
}

function replaceOnce(contents, pattern, replacement, label) {
  const updated = contents.replace(pattern, replacement);
  if (updated === contents) {
    throw new Error(`No se encontró la versión en ${label}`);
  }
  return updated;
}

const packageJson = JSON.parse(read(packageJsonPath));
const currentVersion = packageJson.version;
const nextVersion = bumpPatch(currentVersion);

const packageJsonText = replaceOnce(
  read(packageJsonPath),
  /("version"\s*:\s*)"[^"]+"/,
  `$1"${nextVersion}"`,
  "package.json",
);
const packageLockText = replaceOnce(
  read(packageLockPath),
  /("version"\s*:\s*)"[^"]+"/,
  `$1"${nextVersion}"`,
  "package-lock.json (raíz)",
);
const packageLockWithRootVersion = replaceOnce(
  packageLockText,
  /(\"\"\s*:\s*\{[\s\S]*?\"name\"\s*:\s*\"nightdesk\"\s*,\s*\n\s*\"version\"\s*:\s*)\"[^\"]+\"/,
  `$1"${nextVersion}"`,
  "package-lock.json (packages[\"\"])",
);
const tauriConfigText = replaceOnce(
  read(tauriConfigPath),
  /("version"\s*:\s*)"[^"]+"/,
  `$1"${nextVersion}"`,
  "src-tauri/tauri.conf.json",
);
const cargoTomlText = replaceOnce(
  read(cargoTomlPath),
  /(^version\s*=\s*)"[^"]+"/m,
  `$1"${nextVersion}"`,
  "src-tauri/Cargo.toml",
);
const cargoLockText = replaceOnce(
  read(cargoLockPath),
  /(\[\[package\]\]\s*\nname = "nightdesk"\s*\nversion = )"[^"]+"/,
  `$1"${nextVersion}"`,
  "src-tauri/Cargo.lock",
);

write(packageJsonPath, packageJsonText);
write(packageLockPath, packageLockWithRootVersion);
write(tauriConfigPath, tauriConfigText);
write(cargoTomlPath, cargoTomlText);
write(cargoLockPath, cargoLockText);

console.log(`Versión actualizada: ${currentVersion} → ${nextVersion}`);
