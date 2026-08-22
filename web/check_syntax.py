"""Validate JavaScript syntax of inline <script> blocks in index.html.

Extracts every inline (src-less) <script> block and asks Node to compile
it with `new Function(...)`. Node's real parser handles regex literals,
template strings, and comments correctly, unlike naive brace counting.

Usage: python3 check_syntax.py   (run from the web/ directory)
"""
import re
import subprocess
import sys

HTML = "index.html"

with open(HTML, encoding="utf-8") as f:
    src = f.read()

blocks = []
for m in re.finditer(r"<script\b([^>]*)>(.*?)</script>", src, re.DOTALL | re.IGNORECASE):
    attrs = m.group(1)
    if re.search(r"\bsrc\s*=", attrs, re.IGNORECASE):
        continue
    if re.search(r"\btype\s*=\s*['\"](?:application/(?:ld\+)?json|text/template)", attrs, re.IGNORECASE):
        continue
    content = m.group(2).strip()
    if content:
        blocks.append((m.start(), content))

if not blocks:
    print("FAIL: no inline script blocks found")
    sys.exit(1)

failed = 0
for idx, (offset, content) in enumerate(blocks, 1):
    line = src[:offset].count("\n") + 1
    # Pass the script via stdin to avoid shell/argv escaping issues.
    wrapper = "const s = require('fs').readFileSync(0, 'utf8');\nnew Function(s);"
    r = subprocess.run(
        ["node", "-e", wrapper],
        input=content,
        capture_output=True,
        text=True,
    )
    if r.returncode == 0:
        print("OK: script block %d (html line %d, %d chars)" % (idx, line, len(content)))
    else:
        failed += 1
        err = (r.stderr or "").strip().splitlines()
        print("FAIL: script block %d (html line %d)" % (idx, line))
        for ln in err[:6]:
            print("   ", ln)

if failed:
    print("RESULT: %d of %d script blocks failed" % (failed, len(blocks)))
    sys.exit(1)
print("RESULT: all %d script blocks parse cleanly" % len(blocks))
