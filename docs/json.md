# The JSON report

The machine contract: other tools read it, so its fields change only with this file.

Without `--code`, `--tests` or `--docs`, `--json` prints one object, the three numbers and the warnings:

- `code`, `docs`, `tests`: the three numbers (for `commit` and `pre-commit`, the deltas), each rounded to two
  decimals;
- `warnings`: as a report's, below;
- for `commit` and `pre-commit`, also `from` and `to`, as a report's, below.

```json
{
  "from": "60baba96b16d35eb98e1c87993574780995f624c",
  "to": "b48be68e77d42c8c9a8a28f2060d0bbe261779d4",
  "code": 6.0,
  "docs": 17.0,
  "tests": -5.0,
  "warnings": []
}
```

With `--code`, `--tests` or `--docs`, `--json` prints that number's report, one JSON object with the same lines as
the text, in the same order, `--top` and `--by` acting alike; the whole report, `--verbose` or not. Every field
below is always there, `null` where it has no value. A weight or a delta is a number rounded to two decimals, as the
text shows it.

- `total`: the whole weight (or delta), not only the rows shown.
- `by`: `"function"` or `"file"`.
- `rows`: the report's lines. By file, `{path, weight}`; by function, `{path, kind, name, line, weight}`, `kind`
  one of `"function"`, `"class"` (its code outside its functions) and `"file"` (a file's code outside any class, its
  `name` and `line` `null`), `name` qualified (`Outer.Inner.method`), `line` the first line, from 1 (for `commit`, the after
side's, a removed one's the before side's).
- `warnings`: the files with syntax errors, `{path, kind: "syntax_errors", side}`, `side` `null` for a scan (a
  Markdown file never has one); then, for a scan, the paths under a folder given that couldn't be read and were left out,
  `{path, kind: "unreadable", side: null}`.

For `commit`, also `from` (the full hash compared from, `null` for the root commit's empty tree) and `to` (the full
hash compared to, `null` for `pre-commit`'s uncommitted changes); each row has, instead of `weight`, `before`, `after` (0 on the
side it isn't), `delta` and `status` (`"added"`, `"removed"`, `"heavier"` or `"lighter"`), and `old_path` (the path
before a rename, else `null`); a warning's `side` is `"before"` or `"after"`.

```json
{
  "from": "60baba96b16d35eb98e1c87993574780995f624c",
  "to": "b48be68e77d42c8c9a8a28f2060d0bbe261779d4",
  "total": 6.0,
  "by": "function",
  "rows": [
    {"path": "a.py", "old_path": null, "kind": "function", "name": "f", "line": 1,
     "before": 4.5, "after": 10.5, "delta": 6.0, "status": "heavier"}
  ],
  "warnings": []
}
```
