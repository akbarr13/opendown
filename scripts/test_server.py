# test_server.py — Range-capable HTTP server for OpenDown engine tests.
# usage: python test_server.py FILE [--port 8123] [--no-range] [--throttle 0.02]
import argparse
import http.server
import os
import re
import time


class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    file_path = None
    supports_ranges = True
    throttle = 0.0

    def log_message(self, *a):
        pass

    def handle(self):
        try:
            super().handle()
        except (ConnectionResetError, ConnectionAbortedError, BrokenPipeError, OSError):
            pass

    def _serve(self):
        size = os.path.getsize(self.file_path)
        rng = self.headers.get("Range")
        start, end, status = 0, size - 1, 200
        if rng and self.supports_ranges:
            m = re.match(r"bytes=(\d+)-(\d*)$", rng.strip())
            if m:
                start = int(m.group(1))
                end = int(m.group(2)) if m.group(2) else size - 1
                end = min(end, size - 1)
                status = 206
        length = end - start + 1
        self.send_response(status)
        self.send_header("Content-Length", str(length))
        self.send_header("Content-Type", "application/octet-stream")
        if self.supports_ranges:
            self.send_header("Accept-Ranges", "bytes")
        if status == 206:
            self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
        self.send_header("ETag", '"rs-test"')
        self.end_headers()
        if self.command == "HEAD":
            return
        with open(self.file_path, "rb") as f:
            f.seek(start)
            rem = length
            while rem > 0:
                chunk = f.read(min(262144, rem))
                if not chunk:
                    break
                try:
                    self.wfile.write(chunk)
                except OSError:
                    return
                rem -= len(chunk)
                if self.throttle:
                    time.sleep(self.throttle)

    def do_HEAD(self):
        self._serve()

    def do_GET(self):
        self._serve()


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("file")
    ap.add_argument("--port", type=int, default=8123)
    ap.add_argument("--no-range", action="store_true")
    ap.add_argument("--throttle", type=float, default=0.0)
    a = ap.parse_args()
    cls = type("HH", (H,), {
        "file_path": a.file,
        "supports_ranges": not a.no_range,
        "throttle": a.throttle,
    })
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", a.port), cls)
    print(f"serving {a.file} on http://127.0.0.1:{a.port} (range={not a.no_range})", flush=True)
    srv.serve_forever()
