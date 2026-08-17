#!/usr/bin/env python3
"""Which of the spec's 29 acceptance criteria are cited by a *test*?

Run from the repository root:

    python3 tools/criteria_audit.py

Exits non-zero, and names the criteria, if any of the 29 is not cited from
inside a `#[test]` function.

Why this exists rather than `grep -rn criterion crates/`: that grep counts a
module-level `//!` doc comment and a `///` doc comment on a plain function as
coverage. Four criteria (13, 19, 28, 29) were resting on exactly that, and the
grep reported them as covered. If you change this script, keep the distinction:
a citation only counts when the item it belongs to carries `#[test]`.

The two comment positions need opposite searches, which is the other thing the
naive version gets wrong:

  - a `///` doc comment sits *above* the item it documents, so the owning `fn`
    is the next one *below* the citation;
  - a `//` comment inside a body has its owning `fn` *above* it.
"""

import collections
import os
import re
import sys

CRITERIA = 29
CITATION = re.compile(r"criteri\w*\s+((?:\d+\s*(?:,|and|/)?\s*)+)")
FN = re.compile(r"\s*(?:pub )?fn (\w+)")


def owning_test(lines, i):
    """The name of the `#[test]` function the citation on line `i` belongs to."""
    line = lines[i].strip()

    if line.startswith("///"):
        # Scan down past the rest of the doc comment and any attributes.
        j = i
        while j < len(lines):
            if FN.match(lines[j]):
                name = FN.match(lines[j]).group(1)
                return name if any("#[test]" in lines[k] for k in range(i, j + 1)) else None
            stripped = lines[j].strip()
            if not (stripped.startswith("///") or stripped.startswith("#[") or stripped == ""):
                return None
            j += 1
        return None

    if line.startswith("//"):
        # Scan up to the enclosing fn, then check for #[test] above it.
        for j in range(i, -1, -1):
            if FN.match(lines[j]):
                k = j - 1
                while k >= 0 and (lines[k].strip().startswith("//") or not lines[k].strip()):
                    k -= 1
                return FN.match(lines[j]).group(1) if k >= 0 and "#[test]" in lines[k] else None
        return None

    return None


def main():
    covered = collections.defaultdict(set)
    for root, _, files in os.walk("crates"):
        for name in files:
            if not name.endswith(".rs"):
                continue
            path = os.path.join(root, name)
            with open(path, encoding="utf-8") as handle:
                lines = handle.read().split("\n")
            for i, line in enumerate(lines):
                match = CITATION.search(line)
                if not match:
                    continue
                test = owning_test(lines, i)
                for number in re.findall(r"\d+", match.group(1)):
                    if test:
                        covered[int(number)].add(test)

    missing = []
    for criterion in range(1, CRITERIA + 1):
        tests = sorted(covered.get(criterion, ()))
        if tests:
            print(f"{criterion:2d}  {', '.join(tests)}")
        else:
            missing.append(criterion)
            print(f"{criterion:2d}  ** not cited by any test **")

    if missing:
        print(f"\n{len(missing)} criteria uncited by a test: {missing}")
        return 1
    print(f"\nAll {CRITERIA} criteria are cited by at least one test.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
