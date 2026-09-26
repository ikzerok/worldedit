"""Validate research metadata only, not product correctness or UX success."""
from pathlib import Path
import json, re
p=Path(__file__).parent
sources=json.loads((p/'sources.json').read_text(encoding='utf-8'))
obs=json.loads((p/'observations.json').read_text(encoding='utf-8'))
ids={s['id'] for s in sources}
assert len(ids)==len(sources)==22
assert all(s['url'].startswith('https://') and s['accessed_at']=='2026-09-26' for s in sources)
assert all(not s['hands_on'] for s in sources)
text=(p/'guide.md').read_text(encoding='utf-8')
assert set(re.findall(r'\bS\d{2}\b',text)) <= ids
assert len({o['id'] for o in obs})==len(obs)==4
for o in obs:
    assert set(o['source_ids']) <= ids
    assert o['measured_time_ms'] is None
    assert o['validation_needed']
    for pattern in o['pattern_ids']:
        assert pattern in text
for f in ['observation.md','decision.md','ticket.md','pattern.md','study.md']:
    assert (p/'templates'/f).is_file()
if (p/'tickets.json').exists():
    tickets=json.loads((p/'tickets.json').read_text(encoding='utf-8'))
    keys={t['id'] for t in tickets}
    assert len(keys)==len(tickets)
    for t in tickets:
        assert set(t.get('depends_on',[])) <= keys
        assert set(t.get('source_ids',[])) <= ids
    visiting=set(); done=set()
    def visit(k):
        assert k not in visiting, 'dependency cycle: '+k
        if k in done: return
        visiting.add(k)
        for d in next(t for t in tickets if t['id']==k).get('depends_on',[]): visit(d)
        visiting.remove(k); done.add(k)
    for k in keys: visit(k)
print('DOC_METADATA_OK: 22 sources; 4 initial observations; templates and references valid.')
print('NOT a product-test, commercial-GUI walkthrough, accessibility certification, or user-study result.')
