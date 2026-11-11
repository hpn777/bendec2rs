// Development-only oracle. Never included in the public crate or used at runtime.
const fs = require('node:fs');
const path = require('node:path');
const cp = require('node:child_process');
const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const root = path.resolve(__dirname, '..');
const { generateString } = require('./oracle.cjs');
const syntheticOnly = process.argv.includes('--synthetic-only');
assert(syntheticOnly || process.env.BENDEC_SCHEMA_ROOT,
  'Full corpus requires BENDEC_SCHEMA_ROOT; use --synthetic-only explicitly for synthetic coverage');
const types = [
  { name: 'u8', size: 1 }, { name: 'u32', size: 4 }, { name: 'char', size: 1 },
  { name: 'Identifier', alias: 'u32', description: 'Stable ID' },
  { name: 'Label', type: 'u8', length: 40 },
  { name: 'Tag', underlying: 'u8', variants: [['First', 1, 'First kind'], ['Second', '0x02']] },
  { name: 'First', fields: [{ name: 'messageType', type: 'Tag' }, { name: 'HTTPServerID', type: 'Identifier', description: 'ID' }, { name: 'text', type: 'char', length: 16 }] },
  { name: 'Second', fields: [{ name: 'messageType', type: 'Tag' }, { name: 'data', type: 'u8', length: 64 }] },
  { name: 'Message', discriminator: ['messageType'], members: ['First', 'Second'], description: 'Message union' },
];
const cases = [];
function add(name, defs, options = {}, rustOptions = options) {
  cases.push({ name, request: { types: defs, options: rustOptions }, expected: generateString(structuredClone(defs), options) });
}
add('all-kinds', types);
add('empty', []);
add('derives-visibility', types, { camelCase: true, visibilityPrefix: 'pub(crate) ', defaultDerives: { struct: ['Copy', 'Clone', 'Default'] }, extraDerives: { Identifier: ['Debug'] }, meta: { First: { publicFields: ['messageType'], fields: { HTTPServerID: { annotations: ['#[serde(default)]'] } }, privateFields: ['text'], annotations: ['#[allow(dead_code)]'] } } });
for (const kind of ['Public', 'Private', 'InCrate', 'InPath']) {
  add(`newtype-${kind}`, types, { meta: { Identifier: { newtype: { kind, module: 'crate::wire', deref: true, asInner: true, intoInner: true, constr: true } }, Label: { newtype: { kind, module: 'crate::wire' } } } });
}
add('offset', [{ name: 'Code', underlying: 'u32', offset: '10', variants: [['One', '0b01'], ['Two', '0x02'], ['Negative', -4]] }]);
for (const transparentBitflags of [false, true]) {
  add(`bitflags-${transparentBitflags}`, [{ name: 'Flags', underlying: 'u8', variants: [['Read', 1], ['Write', 2]], bitflags: true }], { transparentBitflags, meta: { Flags: { bitflags: true } } });
}
add('constants', types, { meta: { Tag: { implConst: true } } });
add('union-enum', types, { meta: { Message: { union: { underlying: 'Tag', discVariant: 'Tag' } } } });
add('mapping', types, { typeMapping: { Identifier: () => 'u64', 'char[]': size => `[u8; ${size}]` } }, { typeMapping: { Identifier: 'u64', 'char[]': '[u8; {{ size }}]' } });
add('errors-and-extras', types, { extras: ['use std::convert::TryFrom;', '// extra'], enumConversionError: { type: 'InvalidVariant', constructor: 'InvalidVariant::new(other as u32, "{{ name }}")' } });
add('field-names-and-docs', [{ name: 'Names', description: 'Line one\nLine two', fields: ['XMLHttpRequest', 'order2ID', 'already_snake', 'lowerUPPER', 'foo-bar', 'version42Value'].map(name => ({ name, type: 'external.Value', description: 'Field description' })) }]);
add('upstream-numeric-string-quirk', [{ name: 'Code', underlying: 'u8', variants: [['DecimalString', '12'], ['Binary', '0b10'], ['Hex', '0x03']] }]);
if (!syntheticOnly) {
  const messages = path.resolve(process.env.BENDEC_SCHEMA_ROOT);
  const files = fs.readdirSync(messages).filter(f => f.endsWith('.json') && f !== 'schema.json').sort();
  assert(files.length > 0, 'BENDEC_SCHEMA_ROOT must contain schema JSON files');
  for (const file of files) {
    const defs = JSON.parse(fs.readFileSync(path.join(messages, file))).typeDefinitions;
    assert(defs && typeof defs === 'object', `${file}: missing typeDefinitions`);
    add(file, Array.isArray(defs) ? defs : Object.values(defs).flat());
  }
}
// Fixture updates are explicit and always sourced from the pinned JS oracle.
if (process.argv.includes('--write-fixtures')) {
  assert(process.argv.includes('--synthetic-only'));
  fs.writeFileSync(path.join(root, 'tests/fixtures.json'), JSON.stringify(cases, null, 2) + '\n');
  if (process.argv.includes('--fixtures-only')) process.exit(0);
}
const receipt = process.env.BENDEC_RECEIPTS;
if (receipt) {
  fs.mkdirSync(receipt, { recursive: true });
  fs.writeFileSync(path.join(receipt, 'corpus.json'), JSON.stringify(cases.map(c => c.request)));
  fs.writeFileSync(path.join(receipt, 'manifest.json'), JSON.stringify({ oracle: 'bendec@0.52.0', cases: cases.map(c => ({ name: c.name, types: c.request.types.length, sha256: createHash('sha256').update(c.expected).digest('hex') })) }, null, 2));
}
if (process.argv.includes('--prepare-only')) {
  assert(receipt, '--prepare-only requires BENDEC_RECEIPTS');
  process.exit(0);
}
const binary = process.env.BENDEC_BINARY || path.join(root, 'target/release/bendec2rs');
const input = cases.map(c => JSON.stringify(c.request)).join('\n') + '\n';
const start = performance.now();
const result = cp.spawnSync(binary, ['--batch'], { input, encoding: 'utf8', maxBuffer: 128 * 1024 * 1024 });
assert.equal(result.status, 0, result.error?.message || result.stderr);
const actual = result.stdout.trimEnd().split('\n').map(s => JSON.parse(s));
assert.equal(actual.length, cases.length);
let failed = 0;
for (let i = 0; i < cases.length; i++) {
  if (actual[i] !== cases[i].expected) {
    failed++;
    console.error(`FAIL ${cases[i].name}`);
    if (receipt) {
      fs.writeFileSync(path.join(receipt, `${i}.expected.rs`), cases[i].expected);
      fs.writeFileSync(path.join(receipt, `${i}.actual.rs`), actual[i]);
    }
  }
}
console.log(JSON.stringify({ cases: cases.length, failed, types: cases.reduce((n, c) => n + c.request.types.length, 0), outputBytes: cases.reduce((n, c) => n + Buffer.byteLength(c.expected), 0), rustBatchWallMsIncludingStartupAndIPC: performance.now() - start }));
if (failed) process.exitCode = 1;
