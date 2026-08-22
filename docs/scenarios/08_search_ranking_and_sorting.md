# Search Ranking & Sort Determinism

**51. Hardcoded Relevance Overrode User Sorting**
*The Pitfall (Deep Context):*
The frontend offered a proud little dropdown: "Relevance", "Title A-Z", "Newest First". Users clicked "Title A-Z" and watched the results come back in exactly the same order as before. Not roughly similar. Identical. For weeks the complaint sounded like a frontend bug, a stale cache, anything except what it actually was.
The truth lived in `server.rs`. The search handler constructed its `ORDER BY` clause inside an `if/else` chain, and one branch had absolute authority: the moment a text query `q` was present, the clause was hardcoded to `ORDER BY bm25(...)`. The `sort` URL parameter was parsed, carried around politely, and then silently thrown away whenever a text query existed. Relevance was not the default. It was a dictatorship.

*How we faced it:*
We replaced the entire `if/else` ladder with an exhaustive `match` over a single tuple, `(has_text_q, params.sort.as_deref())`, so the compiler forced us to declare who wins in every combination:
```rust
let order_by = match (has_text_q, params.sort.as_deref()) {
    // Explicit user choice ALWAYS wins, text query or not.
    (_, Some("title_asc"))  => "ORDER BY p.course_title COLLATE NOCASE ASC, p.id ASC",
    (_, Some("title_desc")) => "ORDER BY p.course_title COLLATE NOCASE DESC, p.id ASC",
    // bm25 relevance is only the fallback when the user never chose a sort.
    (true, _) => "ORDER BY bm25(papers_fts) ASC, p.id ASC",
    _ => "ORDER BY p.id ASC",
};
```
The semantic rule became trivially auditable: an explicit sort parameter outranks everything, and relevance exists only in the fallback arm. The dropdown started telling the truth the same hour we redeployed.

-> *Verify: `curl 'http://localhost/api/search?q=thermodynamics&sort=title_asc&limit=3'` returns titles in ascending alphabetical order despite the presence of a text query.*


**52. Unstable Pagination Without Tiebreakers**
*The Pitfall (Deep Context):*
Fixing the sort selector exposed a deeper rot. With `sort=title_asc` active, paging through results felt haunted. In one request, "General Chemistry I" sat as the very last row of page 1. Reload, advance again, and the same paper appeared as the very first row of page 2. Rows duplicated on one pass and vanished on the next, purely depending on what we came to call SQLite's scan mood.
The cause was embarrassingly fundamental: our 19,600 rows contain enormous runs of duplicate sort keys, dozens of papers sharing the identical title. An `ORDER BY` with no final tiebreaker defines no total ordering, so SQLite is free to break ties however its query plan happens to walk the table that day. Any `LIMIT/OFFSET` window sliced across tied keys is therefore non-deterministic. Pagination without a tiebreaker is not pagination. It is roulette.

*How we faced it:*
We made the ordering total by appending `, p.id ASC` as the final expression of all five `ORDER BY` branches: `title_asc`, `title_desc`, newest-first, oldest-first, and the bm25 relevance fallback. Row IDs are unique, so once the human-facing keys tie, the id imposes a strict, stable sequence. Page boundaries can no longer shuffle between requests because there is no longer any freedom left for the engine to exercise.

-> *Verify: Running `for o in 0 5000 10000 19580; do curl -s "http://localhost/api/search?sort=title_asc&limit=20&offset=$o" | md5sum; done` twice produces byte-identical hash sequences, and consecutive pages share zero overlapping rows.*


**53. The Fix Sat in Git While Production Stayed Broken**
*The Pitfall (Deep Context):*
The sort fix was committed, pushed, and blessed by a green CI pipeline. We moved on. Hours later a user reported the exact same broken ordering we had "fixed" that morning. Confusion curdled into suspicion: was our diagnosis wrong? No. The diagnosis was fine. The production binary had simply never heard of it.
We had been spoiled by our own frontend. Cloudflare Pages rebuilds and ships the site automatically on every push, so "merged means deployed" had quietly become muscle memory. But the Rust API is not a static site. It is a compiled binary living on an Oracle VM, and CI passing proves nothing about that machine. Nobody had run a single build on the box. The correct code existed only in the repository while the live process kept serving the old logic, and every minute of debugging the "still-broken" API was spent interrogating code that production was not even running.

*How we faced it:*
We wrote down the deployment ritual as an explicit, mandatory sequence instead of an assumption. Sources go up, the binary is rebuilt under a memory-strangled toolchain, and the daemon is bounced around the copy:
```bash
scp -r src/ Cargo.toml opc@68.233.111.2:~/app/
ssh opc@68.233.111.2 "cd ~/app && cargo build --release --bin server -j 1"
ssh opc@68.233.111.2 "sudo systemctl stop amrita-server && \
  sudo cp ~/app/target/release/server /usr/local/bin/amrita-server && \
  sudo systemctl start amrita-server"
```
Two hard-won details are baked in. `-j 1` exists because the VM has 498MB of RAM and parallel linking OOM-kills the build. And the stop/copy/start dance exists because copying over a running executable fails with `Text file busy`; the kernel refuses to mutate a binary mid-execution. A merged PR is now treated as "ready to deploy", never as "deployed".

-> *Verify: `ssh opc@68.233.111.2 'stat -c %Y /usr/local/bin/amrita-server'` reports a modification time newer than the fix commit's timestamp, and `curl -s 'http://localhost/api/search?sort=title_asc&limit=1'` exhibits the new ordering.*


**54. Displaying Sanitized Titles While Sorting Raw Garbage**
*The Pitfall (Deep Context):*
Even after scenarios 51 through 53, Title A-Z still looked subtly scrambled. Titles beginning with clean letters were interrupted by ghosts. This was the deepest bug in the saga, and it hid in plain sight inside `index.db`.
The `course_title` column does not hold clean strings. It holds raw `pdftotext` scrapings, complete with junk prefixes fused onto real titles: `"& 21RM610 Research Methodology"`, `", 002 and 111 reflections."`. Our API sanitizes every title at read time through `sanitize_title()` before display, so users always saw polished labels. But the `ORDER BY` never touched `sanitize_title()`. It sorted the raw, polluted column directly. Punctuation and embedded digits dictated alphabetical position while the UI showed pristine text, producing an order that looked random because its input was invisible. We were sorting one dataset and displaying another.

*How we faced it:*
If display sanitizes, sorting must sanitize identically. `rusqlite` lets you register Rust functions as SQLite scalar functions, so we taught the database itself to clean titles. This required enabling the `functions` feature in `Cargo.toml` (`rusqlite = { version = "0.31", features = ["bundled", "functions"] }`), then registering the sanitizer inside the `r2d2` connection pool's `with_init` closure so every pooled connection receives it, not just the first one:
```rust
pool = r2d2::Pool::builder()
    .max_size(4)
    .build_unchecked(manager_with_init(|conn| {
        conn.create_scalar_function("clean_title", 2, move |ctx| {
            let raw: String = ctx.get(0)?;
            let code: String = ctx.get(1)?;
            Ok(sanitize_title(&raw, &code))
        })
    }))?;
```
With the function living inside the pool, the sort clause finally operated on the same strings users see: `ORDER BY clean_title(p.course_title, p.course_code) COLLATE NOCASE ASC, p.id ASC`. One definition, one truth, shared by display and ordering alike.

-> *Verify: `sqlite3 index.db "SELECT course_title FROM papers WHERE course_title LIKE '&%' LIMIT 3"` confirms raw junk exists in storage, while `curl -s 'http://localhost/api/search?sort=title_asc&limit=3'` returns rows ordered by their cleaned titles, ignoring leading punctuation.*


**55. COLLATE Placed After ASC Is a Syntax Error**
*The Pitfall (Deep Context):*
While wiring case-insensitive ordering into the new `clean_title` clauses, we wrote what read naturally in English: `ORDER BY expr ASC COLLATE NOCASE`. It looks reasonable. Every human reading it understands the intent. SQLite does not care. Its grammar demands that a collation attach to the expression before the direction, so the parser hit the dangling keyword and rejected the entire statement.
Because this shipped inside a release build, the failure mode was spectacular: every search request, sorted or unsorted, returned `near "COLLATE": syntax error`. We had converted a subtle ordering quirk into a total outage of the search endpoint, discovered only after deploying and watching previously working queries die instantly.

*How we faced it:*
The fix is one token of reordering, but the lesson is permanent: in SQLite, `COLLATE` binds tighter than the direction keyword and must precede it. The clause went from invalid to valid:
```sql
-- rejected: ORDER BY clean_title(p.course_title, p.course_code) ASC COLLATE NOCASE
ORDER BY clean_title(p.course_title, p.course_code) COLLATE NOCASE ASC, p.id ASC
```
We now smoke-test every newly composed SQL fragment against a real `sqlite3` shell before it is allowed anywhere near a release binary. Grammar errors are cheap to catch locally and expensive to catch in production.

-> *Verify: `curl 'http://localhost/api/search?sort=title_asc&limit=3'` returns a JSON result array instead of `near "COLLATE": syntax error`.*


**56. Digit-Led Fallback Titles Stormed the Top of Title A-Z**
*The Pitfall (Deep Context):*
Sorting cleaned titles surfaced a population nobody had counted: papers whose PDF text extraction failed entirely. Their display falls back to `"<CODE> Examination Paper"`, a synthetic label built from the course code. Functional, honest, and catastrophic for alphabetical order, because those codes start with digits, and digits sort before letters. The instant raw-garbage sorting was fixed, every extraction-failure paper in the archive leapt to the front of Title A-Z. Students opening the list were greeted by pages of "210EXAM Examination Paper" before reaching a single real title beginning with A.
Our first fix tried to be clever: identify fallback rows by comparing `clean_title` against the reconstructed string `course_code || ' Examination Paper'`. It passed casual testing and failed in the field, because the code embedded in a scraped title does not always match the stored `course_code` column. String-equality detection assumed a consistency the data never promised.

*How we faced it:*
We stopped trying to classify fallbacks by their content and classified them by their visible property instead: their first character is a digit. One `substr` probe sinks the entire population to the bottom of every listing regardless of how their codes diverge:
```sql
ORDER BY
  CASE WHEN substr(clean_title(p.course_title, p.course_code), 1, 1)
       BETWEEN '0' AND '9' THEN 1 ELSE 0 END ASC,
  clean_title(p.course_title, p.course_code) COLLATE NOCASE ASC,
  p.id ASC
```
Real titles now own the top of the list in true alphabetical order, and every digit-led fallback parks at the tail where it belongs: present, findable, and no longer shouting first.

-> *Verify: `curl -s 'http://localhost/api/search?sort=title_asc&limit=5&offset=19585'` returns only digit-led `<CODE> Examination Paper` entries at the tail, while `limit=5&offset=0` returns no digit-led titles at all.*


**57. The Binary Name That Never Existed**
*The Pitfall (Deep Context):*
Mid-deploy on the RAM-starved Oracle box, we confidently typed `cargo build --release --bin amrita-server`. Where did the name come from? The systemd unit: `amrita-server.service`. If the service is called that, surely the binary is too. Cargo answered immediately and mercilessly: `error: no bin target named amrita-server`. We had invented a target name by transitive reasoning and burned a full compile cycle on a 498MB machine learning nothing.
The actual truth required opening `Cargo.toml`, where the `[[bin]]` section declares the target as plain `server`. The systemd unit name was always just a label for the service wrapper, bearing no contractual relationship to the artifact Cargo produces. Two naming systems that happen to coexist in the same project had been conflated into one false assumption.

*How we faced it:*
We adopted a pre-flight cross-check rule for every remote build: before invoking cargo, confirm the exact `[[bin]]` names in `Cargo.toml`, and confirm the systemd unit's `ExecStart=` points at precisely that artifact path. The unit may keep any name it likes, but the `ExecStart` line must reference the binary Cargo actually emits (`/usr/local/bin/amrita-server` in our case). Thirty seconds of grepping beats twenty minutes of watching a constrained machine link a target that was never defined.

-> *Verify: `grep -A1 '\[\[bin\]\]' Cargo.toml` lists exactly the bin name passed to `cargo build --bin`, and `systemctl cat amrita-server | grep ExecStart` references that same binary path.*


**58. Proving Sort Correctness End-to-End**
*The Pitfall (Deep Context):*
Five successive sort bugs taught us that eyeballing a single page proves nothing. Ordering defects hide at page boundaries, in tie groups, at the extremes of 19,600 rows, and in the gap between what the database stores and what the UI renders. Any claim like "sorting works now" was worthless unless a mechanical loop could demonstrate it against the live API, repeatedly, without human optimism in the loop.

*How we faced it:*
We built a three-layer verification ritual and ran it after every change. First, fingerprints: hashing full responses across sort modes proves the parameter genuinely alters output, since a dead sort would yield identical hashes. Second, sequences: parse JSON and print actual title orderings instead of trusting status codes. Third, extremes: assert the very first page opens with A-titles and the deepest offset ends among digit-led fallbacks.
```bash
for s in "" title_asc title_desc; do \
  curl -s "http://localhost/api/search?sort=$s&limit=200" | md5sum; done
curl -s 'http://localhost/api/search?sort=title_asc&limit=3' | \
  python3 -c 'import sys,json; [print(r["course_title"]) for r in json.load(sys.stdin)]'
curl -s 'http://localhost/api/search?sort=title_asc&limit=3&offset=19590' | \
  python3 -c 'import sys,json; [print(r["course_title"]) for r in json.load(sys.stdin)]'
```
The observed output sealed the saga. The first page printed `A 802 11 G.`, then `Accountancy`, then `Achieving Excellence.` in flawless order. The final page at `offset=19590` ended with digit-led fallback papers such as `25EEXXX Examination Paper` resting exactly where scenario 56 promised to sink them. Three hashes differed, two extremes matched theory, and for the first time the dropdown, the database, and reality agreed.

-> *Verify: The loop above prints three distinct md5 hashes across sort modes, an opening sequence led by `A 802 11 G.`, and a closing sequence dominated by digit-led `Examination Paper` fallbacks.*
