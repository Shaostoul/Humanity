// A static file server for the pages in this folder, bound to the loopback
// address only.
//
// It exists to answer one question the spike was asked: does a platform's
// embedded player accept a page served from http://localhost, as opposed to a
// page opened from a file on disk? Twitch in particular requires the embedding
// page's host name to be declared, and "localhost" is the value its own
// documentation gives developers. If localhost works, the watch page can ship
// inside the app and be served by the app's own server, and our website is not
// in the path at all.
//
// The app already contains an HTTP server (src/relay). This script stands in
// for it so the question can be answered without booting the game.
//
// Usage:  node serve.js [port]        (default 8788)

const http = require("http");
const fs = require("fs");
const path = require("path");

const port = Number(process.argv[2] || 8788);
const root = path.join(__dirname, "pages");

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".png": "image/png",
  ".json": "application/json; charset=utf-8",
};

const server = http.createServer((req, res) => {
  // Strip the query string, then refuse anything that tries to climb out of
  // the pages folder.
  const urlPath = decodeURIComponent(req.url.split("?")[0]);
  const rel = urlPath === "/" ? "/anim.html" : urlPath;
  const full = path.join(root, rel);
  if (!full.startsWith(root)) {
    res.writeHead(403).end("no");
    return;
  }
  fs.readFile(full, (err, buf) => {
    if (err) {
      res.writeHead(404, { "content-type": "text/plain" }).end("not found: " + rel);
      return;
    }
    res.writeHead(200, {
      "content-type": TYPES[path.extname(full)] || "application/octet-stream",
      "cache-control": "no-store",
    });
    res.end(buf);
  });
});

// 127.0.0.1 only: this serves test fixtures, not anything the network should
// be able to reach.
server.listen(port, "127.0.0.1", () => {
  console.log(`[serve] http://localhost:${port}/  (root: ${root})`);
});
