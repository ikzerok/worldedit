"""Validate product design metadata, not product correctness or UX success."""
from pathlib import Path
import json, re

p = Path(__file__).parent
sources = json.loads((p / "sources.json").read_text(encoding="utf-8"))
ids = {source["id"] for source in sources}
assert ids == {"S19", "S20", "S21", "S22"} and len(sources) == 4
assert all(source["url"].startswith("https://www.w3.org/") for source in sources)
text = (p / "guide.md").read_text(encoding="utf-8")
assert set(re.findall(r"\bS\d{2}\b", text)) <= ids
for name in ["observation.md", "decision.md", "ticket.md", "pattern.md", "study.md"]:
    assert (p / "templates" / name).is_file()
tickets = json.loads((p / "tickets.json").read_text(encoding="utf-8"))
keys = {ticket["id"] for ticket in tickets}
assert len(keys) == len(tickets)
for ticket in tickets:
    assert set(ticket.get("depends_on", [])) <= keys
    assert set(ticket.get("source_ids", [])) <= ids
visiting, done = set(), set()

def visit(key):
    assert key not in visiting, "dependency cycle: " + key
    if key in done:
        return
    visiting.add(key)
    for dependency in next(t for t in tickets if t["id"] == key).get("depends_on", []):
        visit(dependency)
    visiting.remove(key)
    done.add(key)

for key in keys:
    visit(key)
print("DESIGN_METADATA_OK: 4 technical standards; templates and ticket references valid.")
print("NOT a product-test, accessibility certification, or user-study result.")
