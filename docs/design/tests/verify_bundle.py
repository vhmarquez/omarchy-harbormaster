#!/usr/bin/env python3
"""Dependency-free verification of frozen assets and local study structure.

Run from anywhere. This script never modifies the bundle and never connects to
agents, websites or the desktop. Required checks fail rather than silently skip.
"""
from collections import Counter
from hashlib import sha256
from html.parser import HTMLParser
from pathlib import Path
import json
import re

ROOT = Path(__file__).resolve().parents[1]
inventory = json.loads((ROOT / 'original-inventory.json').read_text())
entries = inventory['files']
assert len(entries) == inventory['file_count'] == 23
assert len({e['path'] for e in entries}) == 23
actual = {p.relative_to(ROOT / 'original').as_posix()
          for p in (ROOT / 'original').rglob('*') if p.is_file()}
assert actual == {e['path'] for e in entries}, 'original file set changed'
for entry in entries:
    content = (ROOT / 'original' / entry['path']).read_bytes()
    assert len(content) == entry['bytes'], entry['path']
    assert sha256(content).hexdigest() == entry['sha256'], entry['path']
assert 'SIL OPEN FONT LICENSE Version 1.1' in (ROOT / 'original/assets/OFL.txt').read_text()
assert inventory['selected_option'] == '02 Project manager'

class StudyParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.ids = []
        self.links = []
        self.scripts = []
        self.recovery = False
        self.recovery_count = 0

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if 'id' in a:
            self.ids.append(a['id'])
        if tag == 'a' and 'href' in a:
            self.links.append(a['href'])
        if tag == 'script':
            self.scripts.append(a)
        if tag == 'select' and a.get('id') == 'recovery-state':
            self.recovery = True
        if tag == 'option' and self.recovery:
            self.recovery_count += 1

    def handle_endtag(self, tag):
        if tag == 'select':
            self.recovery = False

html = (ROOT / 'supplemental.html').read_text()
parser = StudyParser()
parser.feed(html)
assert not [key for key, n in Counter(parser.ids).items() if n > 1], 'duplicate HTML IDs'
assert len(parser.scripts) == 2 and all('src' not in s for s in parser.scripts)
for link in parser.links:
    assert not re.match(r'^[a-z]+:', link), f'external study link: {link}'
    target = (ROOT / link.split('#')[0]).resolve()
    assert target.is_relative_to(ROOT) and target.exists(), f'missing/escaping link: {link}'
assert parser.recovery_count == 18
assert html.rstrip().endswith('</body></html>')
assert 'Not a security sandbox.' in html
assert 'grid-template-columns:minmax(0,1fr) max-content;gap:12px' in html
assert 'prefers-reduced-motion:reduce' in html
assert 'max-width:1100px' in html
assert not re.search(r'\b(fetch|XMLHttpRequest|WebSocket|localStorage|sessionStorage)\b', html)
# WCAG relative luminance checks for the opaque illustrative CSS palettes.
# These are source-token checks, not a rendered/native accessibility audit.
def luminance(color):
    channels = [int(color[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    linear = [c / 12.92 if c <= .04045 else ((c + .055) / 1.055) ** 2.4
              for c in channels]
    return sum(c * w for c, w in zip(linear, (.2126, .7152, .0722)))

def contrast(a, b):
    x, y = sorted((luminance(a), luminance(b)))
    return (y + .05) / (x + .05)

contrast_minima = {}
for name, pattern in [('dark', r'body\{(.*?)\}'), ('light', r'body.light\{(.*?)\}')]:
    match = re.search(pattern, html)
    assert match, f'missing {name} palette'
    palette = dict(re.findall(r'--([a-z]+):(#[0-9a-f]{6})', match[1]))
    minima = {role: min(contrast(palette[role], palette[bg])
                        for bg in ('base', 'bg', 'panel', 'sel'))
              for role in ('text', 'muted', 'accent', 'warn', 'bad', 'line')}
    for role, value in minima.items():
        threshold = 3 if role == 'line' else 4.5
        assert value >= threshold, f'{name} {role} contrast {value:.3f} below {threshold}'
    contrast_minima[name] = {k: round(v, 3) for k, v in minima.items()}
print(json.dumps({'original_files': len(entries), 'original_bytes': sum(e['bytes'] for e in entries),
                  'hashes': 'all match', 'font_license': 'OFL retained',
                  'study_inline_scripts': len(parser.scripts),
                  'study_recovery_states': parser.recovery_count,
                  'duplicate_static_ids': 0, 'local_links': 'all resolve',
                  'contrast_minima': contrast_minima,
                  'browser_layout_verified': False}, indent=2))
