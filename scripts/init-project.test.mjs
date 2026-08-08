import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  appendFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { afterEach, test } from "node:test";
import { fileURLToPath } from "node:url";
import { initializeProject, parseArguments } from "./init-project.mjs";

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const createdRoots = [];

function git(root, args) {
  return execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim();
}

function createFixture() {
  const root = mkdtempSync(join(tmpdir(), "arc-project-init-"));
  createdRoots.push(root);
  mkdirSync(join(root, "frontend/public"), { recursive: true });
  mkdirSync(join(root, "backend"), { recursive: true });
  mkdirSync(join(root, "observability"), { recursive: true });
  writeFileSync(
    join(root, "README.md"),
    "<!-- ARC_PROJECT_HEADER_START -->\n# arc-admin\n\nRBAC 框架。\n<!-- ARC_PROJECT_HEADER_END -->\n\n<!-- ARC_TEMPLATE_USAGE_START -->\n## 从模板创建业务项目\n<!-- ARC_TEMPLATE_USAGE_END -->\n",
  );
  writeFileSync(join(root, "FRAMEWORK_VERSION"), "v1.1.0-dev\n");
  writeFileSync(
    join(root, "frontend/public/config.js"),
    "window.__ARC_ADMIN_CONFIG__ = {};\n",
  );
  writeFileSync(
    join(root, "backend/.env.example"),
    'DATABASE_URL="postgres://arc_admin:change-me@localhost:5432/arc_admin"\nJWT_SECRET=change-me\nSERVICE_NAME=arc-admin-backend\n',
  );
  writeFileSync(
    join(root, "observability/.env.example"),
    "COMPOSE_PROJECT_NAME=arc-admin-observability\nGRAFANA_ADMIN_PASSWORD=change-me\n",
  );
  writeFileSync(join(root, ".gitignore"), ".env\n");
  git(root, ["init", "-q"]);
  git(root, ["config", "user.name", "Test User"]);
  git(root, ["config", "user.email", "test@example.test"]);
  git(root, ["add", "."]);
  git(root, ["commit", "-qm", "fixture"]);
  return root;
}

function projectOptions() {
  return parseArguments([
    "--slug",
    "stock-analysis",
    "--title",
    "股票分析系统",
    "--short-name",
    "投研平台",
    "--database",
    "stock_analysis",
    "--permission-prefix",
    "stock",
  ]);
}

afterEach(() => {
  while (createdRoots.length > 0) {
    rmSync(createdRoots.pop(), { recursive: true, force: true });
  }
});

test("initializes a clean template without tracking the local environment", () => {
  const root = createFixture();

  const result = initializeProject(root, projectOptions(), {
    doctorMode: "never",
    secretFactory: () => "a".repeat(64),
    grafanaSecretFactory: () => "g".repeat(32),
  });

  assert.equal(result.frameworkVersion, "v1.1.0-dev");
  const readme = readFileSync(join(root, "README.md"), "utf8");
  assert.match(readme, /# 股票分析系统/);
  assert.doesNotMatch(readme, /从模板创建业务项目/);
  assert.match(
    readFileSync(join(root, "frontend/public/config.js"), "utf8"),
    /stock-analysis-theme/,
  );
  const environment = readFileSync(join(root, "backend/.env"), "utf8");
  assert.match(environment, /\/stock_analysis"/);
  assert.match(environment, new RegExp(`JWT_SECRET=${"a".repeat(64)}`));
  assert.match(environment, /SERVICE_NAME=stock-analysis-backend/);
  const observabilityEnvironment = readFileSync(
    join(root, "observability/.env"),
    "utf8",
  );
  assert.match(
    observabilityEnvironment,
    /COMPOSE_PROJECT_NAME=stock-analysis-observability/,
  );
  assert.match(observabilityEnvironment, /GRAFANA_ADMIN_PASSWORD=g{32}/);
  const metadata = JSON.parse(
    readFileSync(join(root, ".arc-project.json"), "utf8"),
  );
  assert.equal(metadata.project.permissionPrefix, "stock");
  assert.doesNotMatch(git(root, ["status", "--porcelain"]), /backend\/\.env/);
  assert.doesNotMatch(
    git(root, ["status", "--porcelain"]),
    /observability\/\.env/,
  );
});

test("rejects repeated initialization before checking worktree changes", () => {
  const root = createFixture();
  initializeProject(root, projectOptions(), { doctorMode: "never" });

  assert.throws(
    () => initializeProject(root, projectOptions(), { doctorMode: "never" }),
    /项目已经初始化/,
  );
});

test("rejects a dirty worktree and invalid identifiers", () => {
  const root = createFixture();
  appendFileSync(join(root, "README.md"), "\n本地修改\n");

  assert.throws(
    () => initializeProject(root, projectOptions(), { doctorMode: "never" }),
    /必须干净/,
  );
  assert.throws(
    () =>
      parseArguments([
        "--slug",
        "Stock App",
        "--title",
        "股票分析系统",
        "--database",
        "stock-analysis",
        "--permission-prefix",
        "stock:*",
      ]),
    /--slug/,
  );
});

test("refuses to initialize the canonical framework repository", () => {
  const root = createFixture();
  git(root, [
    "remote",
    "add",
    "origin",
    "https://github.com/higoalespn/arc-admin.git",
  ]);

  assert.throws(
    () => initializeProject(root, projectOptions(), { doctorMode: "never" }),
    /框架源仓库/,
  );
});

test("shell entrypoint has valid syntax and exposes Chinese help", () => {
  const entrypoint = join(SCRIPT_DIR, "init-project.sh");
  execFileSync("bash", ["-n", entrypoint]);
  const help = execFileSync("bash", [entrypoint, "--help"], {
    encoding: "utf8",
  });

  assert.match(help, /项目标识/);
  assert.match(help, /--permission-prefix/);
});
