// Static server for web/ with the application/wasm MIME type that
// WebAssembly.instantiateStreaming requires. node tools/serve.mjs [port]
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize } from "node:path";

const root = join(process.cwd(), "web");
const port = Number(process.argv[2] || process.env.PORT || 8070);
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm", ".png": "image/png" };
createServer(async (req, res) => {
  const p = normalize(join(root, new URL(req.url, "http://x").pathname.replace(/^\/$/, "/index.html")));
  if (!p.startsWith(root)) return res.writeHead(403).end();
  let body;
  try {
    body = await readFile(p);   // read first: headers must not be sent before we know it exists
  } catch {
    return res.writeHead(404).end("not found");
  }
  res.writeHead(200, { "Content-Type": types[extname(p)] || "application/octet-stream" }).end(body);
}).listen(port, "127.0.0.1", () => console.log(`raycast-rs at http://127.0.0.1:${port}/`));
