// Run: node --test docs/design/tests/study.test.cjs
// Tests execute the actual inline model and UI in the offline HTML.
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
function model() {
  const file = path.join(__dirname, '../supplemental.html');
  assert.ok(fs.existsSync(file), 'supplemental study must exist');
  const html = fs.readFileSync(file, 'utf8');
  const script = html.match(/<script id="study-model">([\s\S]*?)<\/script>/);
  assert.ok(script, 'the self-contained study must expose its actual model');
  const context = { structuredClone };
  vm.createContext(context);
  vm.runInContext(script[1], context);
  return context.Study;
}
function ui() {
  const html = fs.readFileSync(path.join(__dirname, '../supplemental.html'), 'utf8');
  const nodes = new Map(), documentEvents = new Map(), confirmations = [];
  // DOM boundary only: replacing innerHTML creates fresh controls with the
  // rendered defaults, just as a browser does. No UI/model function is replaced.
  function element(tag, attributes) {
    const attrs = Object.fromEntries([...attributes.matchAll(/([\w-]+)(?:="([^"]*)")?/g)].map(([, key, value]) => [key, value ?? '']));
    let markup = '', children = [];
    const node = {
      tagName: tag.toUpperCase(), id: attrs.id || '', value: '', textContent: '',
      dataset: Object.fromEntries(Object.entries(attrs).filter(([key]) => key.startsWith('data-')).map(([key, value]) => [key.slice(5).replace(/-([a-z])/g, (_, c) => c.toUpperCase()), value])),
      events: new Map(),
      setAttribute(key, value) { attrs[key] = String(value); },
      getAttribute(key) { return attrs[key] ?? null; },
      hasAttribute(key) { return Object.hasOwn(attrs, key); },
      closest(selector) { assert.equal(selector, 'button'); return tag === 'button' ? this : null; },
      addEventListener(type, handler) { this.events.set(type, handler); },
      querySelectorAll(selector) { return children.filter(child => selector.split(',').some(tag => child.tagName === tag.trim().toUpperCase())); },
      get innerHTML() { return markup; },
      set innerHTML(source) {
        for (const child of children) if (child.id) nodes.delete(child.id);
        markup = source;
        children = controls(source);
      }
    };
    if (node.id) nodes.set(node.id, node);
    return node;
  }
  function controls(source) {
    return [...source.matchAll(/<(button|select)\b([^>]*)>([\s\S]*?)<\/\1>|<(input)\b([^>]*)>/g)].map(([, tag, attrs, body = '', input, inputAttrs]) => {
      const node = element(tag || input, attrs || inputAttrs || '');
      node.textContent = body.replace(/<[^>]*>/g, '');
      if (tag === 'select') {
        node.options = [...body.matchAll(/<option\b([^>]*)>([^<]*)<\/option>/g)].map(([, attrs, text]) => ({
          value: attrs.match(/\bvalue="([^"]*)"/)?.[1] ?? text, text, selected: /\bselected\b/.test(attrs)
        }));
        node.value = (node.options.find(option => option.selected) || node.options[0]).value;
      }
      return node;
    });
  }
  const staticHTML = html.split('<script')[0];
  for (const [, tag, attrs] of staticHTML.matchAll(/<([a-z][\w-]*)\b([^>]*\bid="[^"]+"[^>]*)>/g)) element(tag, attrs);
  const buttons = controls(staticHTML).filter(node => node.tagName === 'BUTTON');
  const document = {
    getElementById: id => nodes.get(id) || null,
    querySelectorAll(selector) {
      const attribute = selector.match(/^\[([\w-]+)\]$/);
      assert.ok(attribute, 'unsupported DOM boundary selector: ' + selector);
      return buttons.filter(node => node.hasAttribute(attribute[1]));
    },
    addEventListener(type, handler) { documentEvents.set(type, handler); }
  };
  const context = vm.createContext({ structuredClone, document, window: { confirm(message) { confirmations.push(message); return false; } } });
  for (const [, source] of html.matchAll(/<script[^>]*>([\s\S]*?)<\/script>/g)) vm.runInContext(source, context);
  return {
    get: document.getElementById, confirmations,
    read: expression => vm.runInContext(expression, context),
    nav: (attribute, value) => buttons.find(node => node.getAttribute(attribute) === value),
    settingsButton: attribute => nodes.get('settings-content').querySelectorAll('button').find(node => node.hasAttribute(attribute)),
    click(node) {
      assert.ok(node, 'UI click target must exist');
      const event = { target: node };
      node.onclick?.(event);
      documentEvents.get('click')(event);
    },
    change(id, value) {
      const node = nodes.get(id);
      assert.ok(node.options.some(option => option.value === value), 'draft must be a real option');
      node.value = value;
      nodes.get('settings-content').events.get('change')({ target: node });
    }
  };
}
for (const { group, field, committed, drafts, masks } of [
  { group: 'privacy', field: 'retention-setting', committed: 'state.privacy.retention', drafts: ['7', '90'], masks: ['mask', 'privacy-mask'] },
  { group: 'notifications', field: 'notifications-setting', committed: 'state.settings.notifications', drafts: ['muted', 'snoozed'], masks: ['mask'] },
  { group: 'projects', field: 'workspace-setting', committed: 'state.settings.workspace', drafts: ['checkout'], masks: ['mask'] }
]) {
  for (const draft of drafts) for (const mask of masks) test(`UI masking via #${mask} preserves unapplied ${group} draft ${draft}`, () => {
    const u = ui();
    u.click(u.nav('data-page', 'settings'));
    u.click(u.nav('data-setting', group));
    const original = u.read(committed);
    const unchangedState = 'JSON.stringify({settings:state.settings,retention:state.privacy.retention,capture:state.privacy.capture,sessions:state.sessions,launch:state.launch})';
    const before = u.read(unchangedState);
    assert.equal(u.get(field).value, original);
    assert.equal(u.read('settingsDirty'), false);
    u.change(field, draft);
    assert.equal(u.read('settingsDirty'), true);
    for (const masked of [true, false]) {
      u.click(u.get(mask));
      assert.equal(u.get(field).value, draft, 'presentation masking must preserve the pending form value');
      assert.equal(u.read('settingsDirty'), true, 'presentation masking must preserve the unsaved-edit guard');
      assert.equal(u.read(unchangedState), before, 'masking must not apply edits or alter other model state');
      assert.deepEqual(u.confirmations, [], 'presentation-only masking must not ask to discard');
      assert.equal(u.read('state.privacy.masked'), masked);
      assert.equal(u.get('mask').getAttribute('aria-pressed'), String(masked));
      assert.equal(u.get('mask').textContent, masked ? 'Show text' : 'Hide text now');
      if (group === 'privacy') assert.equal(u.get('privacy-mask').textContent, u.get('mask').textContent);
      assert.equal(u.get('rows').innerHTML.includes('Add parser tests'), !masked);
      assert.equal(u.get('inspector').innerHTML.includes('/fixture/tidepool-parser'), !masked);
      assert.equal(u.get('project').options[2].text, masked ? 'Project 2' : 'Tidepool');
      assert.equal(u.get('project').options[2].value, 'Tidepool');
      if (group === 'projects') assert.equal(u.get('settings-content').innerHTML.includes('/fixture/tidepool'), !masked);
    }
    // Keeping the edit after navigation still uses the real discard guard.
    u.click(u.nav('data-setting', 'harnesses'));
    assert.deepEqual(u.confirmations, ['Discard unsaved study settings?']);
    assert.equal(u.read('setting'), group);
    assert.equal(u.get(field).value, draft);
    assert.equal(u.read('settingsDirty'), true);
    // Only explicit Apply commits the retained draft, including while masked.
    u.click(u.get(mask));
    u.click(u.settingsButton('data-apply'));
    assert.equal(u.read(committed), draft);
    assert.equal(u.get(field).value, draft);
    assert.equal(u.read('settingsDirty'), false);
    // Cancel still restores the last committed value after another mask toggle.
    u.change(field, original);
    u.click(u.get(mask));
    assert.equal(u.get(field).value, original);
    assert.equal(u.read('settingsDirty'), true);
    u.click(u.settingsButton('data-discard'));
    assert.equal(u.get(field).value, draft);
    assert.equal(u.read(committed), draft);
    assert.equal(u.read('settingsDirty'), false);
    u.click(u.get(mask));
    assert.equal(u.get(field).value, draft);
    assert.equal(u.read('settingsDirty'), false, 'masking a clean form must not make it dirty');
  });
}
test('maskable project labels retain stable explicit option values', () => {
  const html = fs.readFileSync(path.join(__dirname, '../supplemental.html'), 'utf8');
  assert.ok(html.includes('<option value="Beacon">Beacon</option>'), 'masking option text must not change project identity');
  assert.ok(html.includes('<option value="Tidepool">Tidepool</option>'));
});
test('portable UI exposes required review/settings/recovery controls without network or storage APIs', () => {
  const html = fs.readFileSync(path.join(__dirname, '../supplemental.html'), 'utf8');
  for (const id of ['theme', 'compact', 'mask', 'settings-page', 'recovery-page', 'review-evidence', 'confirm-dialog', 'command-dialog']) {
    assert.ok(html.includes('id="' + id + '"'), 'missing accessible study control ' + id);
  }
  assert.doesNotMatch(html, /\b(fetch|XMLHttpRequest|WebSocket|localStorage|sessionStorage)\b/);
  assert.doesNotMatch(html, /<(?:script|link)[^>]+(?:src|href)=["']https?:/i);
  for (const [, source] of html.matchAll(/<script[^>]*>([\s\S]*?)<\/script>/g)) new vm.Script(source);
});
test('fixture projection counts overlap intentionally and project scope never changes global totals', () => {
  const m = model(); const s = m.initial();
  assert.equal(s.sessions.length, 6);
  assert.deepEqual(JSON.parse(JSON.stringify(m.counts(s))), {needs:3,running:4,review:1,saved:0});
  assert.deepEqual(JSON.parse(JSON.stringify(m.counts(s, 'Beacon'))), {needs:2,running:2,review:0,saved:0});
  assert.equal(m.counts(s).needs, 3);
});
test('settings are explicit and reversible: hooks need consent and privacy never enables content collection', () => {
  const m = model(); const s = m.initial();
  assert.equal(typeof m.configure, 'function', 'settings reducer required');
  assert.equal(m.configure(s, 'hooks', true, false), false);
  assert.equal(m.configure(s, 'hooks', true, true), true);
  assert.equal(s.settings.hooks, true);
  assert.equal(m.configure(s, 'hooks', false, true), true);
  assert.equal(s.settings.hooks, false);
  assert.equal(m.configure(s, 'notifications', 'muted'), true);
  assert.equal(m.configure(s, 'retention', '7'), true);
  assert.equal(s.privacy.retention, '7');
  assert.equal(m.configure(s, 'retention', 'forever'), false);
  assert.equal(m.configure(s, 'capture', true), false);
  assert.equal(s.privacy.capture, false);
});
test('capability gates distinguish focus, attach, resume and owned termination', () => {
  const m = model(); const s = m.initial();
  assert.equal(typeof m.allowed, 'function', 'shared UI action gate is required');
  assert.equal(m.allowed(s, 's2', 'open-terminal'), true);
  assert.equal(m.allowed(s, 's2', 'resume'), false);
  assert.equal(m.allowed(s, 's3', 'resume'), true);
  assert.equal(m.allowed(s, 's3', 'open-terminal'), false);
  assert.equal(m.allowed(s, 's5', 'end-session'), false);
  assert.equal(m.allowed(s, 's6', 'attach'), false);
  assert.equal(m.allowed(s, 's2', 'approve'), false);
  assert.equal(m.act(s, 'mark-reviewed', 's2'), false);
});
test('launch retry reuses its request and preserves created context; uncertain or duplicate submit cannot spawn', () => {
  const m = model(); const s = m.initial();
  assert.equal(m.launch(s, 'submit'), true);
  const request = s.launch.request;
  assert.equal(s.launch.phase, 'busy');
  assert.equal(m.launch(s, 'submit'), false);
  m.launch(s, 'terminal-failure');
  assert.equal(s.launch.contextCreated, true);
  assert.equal(m.launch(s, 'submit'), false);
  m.launch(s, 'retry-terminal');
  assert.equal(s.launch.request, request);
  assert.equal(s.launch.phase, 'busy');
  m.launch(s, 'success');
  assert.equal(s.launch.phase, 'success');
  assert.equal(s.launch.starts, 1);
});
test('masking hides fixture metadata but does not change retention or erase review state', () => {
  const m = model(); const s = m.initial();
  const before = JSON.stringify(s.sessions);
  assert.equal(s.privacy.capture, false);
  assert.equal(m.visible(s, 'private label'), 'private label');
  m.mask(s, true);
  assert.equal(m.visible(s, 'private label'), 'Hidden');
  assert.equal(s.privacy.retention, '30');
  assert.equal(JSON.stringify(s.sessions), before);
  m.mask(s, false);
  assert.equal(m.visible(s, 'private label'), 'private label');
});
test('opening work or delivering a notification never reviews a turn; explicit review is undoable and does not verify tests', () => {
  const m = model(); const s = m.initial();
  assert.equal(m.counts(s).review, 1);
  m.act(s, 'open-project', 's3');
  m.act(s, 'notification-delivered', 's3');
  assert.equal(m.counts(s).review, 1);
  m.act(s, 'mark-reviewed', 's3');
  assert.equal(m.counts(s).review, 0);
  assert.equal(s.sessions.find(x => x.id === 's3').result, 'Reported');
  m.act(s, 'undo-review', 's3');
  assert.equal(m.counts(s).review, 1);
});
