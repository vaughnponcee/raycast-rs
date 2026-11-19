// Build the wasm module and copy it next to web/index.html.
//   node tools/build-web.mjs      (needs: rustup target add wasm32-unknown-unknown)
import { execFileSync } from "node:child_process";
import { copyFileSync, statSync } from "node:fs";

execFileSync("cargo", ["build", "--release", "--lib", "--target", "wasm32-unknown-unknown"], { stdio: "inherit" });
const src = "target/wasm32-unknown-unknown/release/raycast_rs.wasm";
copyFileSync(src, "web/raycast.wasm");
console.log(`web/raycast.wasm: ${(statSync("web/raycast.wasm").size / 1024).toFixed(1)} KiB`);
