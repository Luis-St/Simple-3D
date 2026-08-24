#!/usr/bin/env python3
"""Regenerate `license.rtf` from the repository's `LICENSE.md`.

The installer's licence page reads RTF, and `LICENSE.md` is the plain-text
PolyForm Noncommercial 1.0.0 with its Markdown headings. Generating one from
the other is what keeps the page shown at install time and the file in the
repository from ever saying different things -- run this after changing
`LICENSE.md`.
"""

import pathlib
import re

root = pathlib.Path(__file__).resolve().parents[2]
text = (root / "LICENSE.md").read_text()

out = [r"{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0\fswiss\fcharset0 Segoe UI;}}", r"\fs18"]
for line in text.split("\n"):
    line = line.rstrip()
    # The Markdown the licence text uses: headings, cross references between
    # sections, the bare URL in angle brackets, code spans and emphasis.
    heading = line.startswith("#")
    line = re.sub(r"^#+\s*", "", line)
    line = re.sub(r"\[([^\]]+)\]\(#[^)]*\)", r"\1", line)
    line = re.sub(r"^<(.*)>$", r"\1", line)
    line = line.replace("***", "").replace("**", "").replace("`", "")
    # RTF's own escapes, then anything non-ASCII as a \uN? escape.
    line = line.replace("\\", "\\\\").replace("{", r"\{").replace("}", r"\}")
    line = "".join(c if ord(c) < 128 else r"\u%d?" % ord(c) for c in line)
    out.append((r"\b " + line + r"\b0\par") if heading else (line + r"\par"))
out.append("}")

(root / "packaging" / "windows" / "license.rtf").write_text("\n".join(out) + "\n")
