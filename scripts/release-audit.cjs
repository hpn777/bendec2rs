// Pre-release differential audit. No fixture updates, builds, or source edits.
// Every JSON case is parsed from the identical input text in both implementations.
const fs = require('node:fs');
const path = require('node:path');
const cp = require('node:child_process');
const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const { generateString } = require('./oracle.cjs');
const root = path.resolve(__dirname, '..');
const binary = process.env.BENDEC_BINARY || path.join(root, 'target/release/bendec2rs');
const receipts = process.env.BENDEC_RECEIPTS;
assert(receipts, 'BENDEC_RECEIPTS is required to retain differential evidence');
fs.mkdirSync(receipts, { recursive: true });
const cases = [];
function add(name, types, options = {}, policy = 'exact') {
  cases.push({ name, policy, input: JSON.stringify({ types, options }) });
}
function field(name, policy = 'exact', options = {}) {
  add(`field-${name}-${options.camelCase ? 'camel' : 'snake'}`,
    [{ name: 'u8', size: 1 }, { name: 'Names', fields: [{ name, type: 'u8' }] }], options, policy);
}
for (const name of [
  'HTTPServerID', 'XMLHttpRequest', 'order2ID', 'version42Value', 'lowerUPPER',
  'already_snake', 'foo-bar', 'foo.bar', 'FooBARBaz', 'ABc', 'a1STb',
  '1stValue', '2ndValue', '3rdValue', '4thValue', '11thValue', '11stValue',
  '2NDValue', 'foo123bar', '123abc', "don'tStop", 'don’tStop',
  'caféValue', 'ÀÉÎÕÜValue', 'ÆtherŒuvre', 'StraßeID', 'ÞingŁódź',
  'İstanbulıValue', 'ŉame', 'e\u0301Value', 'a\u036fB',
]) field(name);
for (const name of [
  'ΔeltaValue', 'ΣigmaHTTP', 'ΑΒΓδεΖ', 'ПриветМир', 'МОЙКод', 'foo😀Bar',
  '👨‍👩‍👧‍👦Name', 'flag🇬🇷Value', '👍🏽ID', '中文Value', 'a\u1ab0B', 'a\u1dc0B',
]) field(name, 'unicode-limit');
// camelCase adds a serde annotation; field identifiers still use snake-case.
for (const name of ['caféValue', 'ПриветМир', 'foo😀Bar', 'e\u0301Value']) {
  field(name, ['ПриветМир', 'foo😀Bar'].includes(name) ? 'unicode-limit' : 'exact', { camelCase: true });
}
// Broad deterministic ASCII coverage, including acronym/digit/ordinal boundaries.
const atoms = ['a', 'A', 'ab', 'AB', 'Ab', 'HTTP', '1', '2', '11', '42', '1st', '2ND', '3rd', '4TH'];
add('ascii-boundary-matrix', [{ name: 'u8', size: 1 }, { name: 'Names', fields:
  atoms.flatMap(a => atoms.map(b => ({ name: `${a}${b}Value`, type: 'u8' }))) }]);
for (const name of ['constructor', 'toString', 'valueOf', 'hasOwnProperty', '__proto__', 'prototype']) {
  add(`prototype-name-${name}`, [{ name, alias: 'u8' }]);
  field(name);
}
const alias = [{ name: 'Id', alias: 'u8' }];
for (const [name, types] of [
  ['null-definition', [null]], ['empty-definition', [{}]],
  ['null-name', [{ name: null, alias: 'u8' }]],
  ['null-fields', [{ name: 'Broken', fields: null }]],
  ['string-fields', [{ name: 'Broken', fields: 'wrong' }]],
  ['null-field', [{ name: 'Broken', fields: [null] }]],
  ['missing-field-type', [{ name: 'Broken', fields: [{ name: 'x' }] }]],
  ['null-description', [{ name: 'Id', alias: 'u8', description: null }]],
  ['null-alias', [{ name: 'Id', alias: null }]],
  ['null-array-length', [{ name: 'Array', type: 'u8', length: null }]],
  ['negative-array-length', [{ name: 'Array', type: 'u8', length: -1 }]],
  ['fractional-array-length', [{ name: 'Array', type: 'u8', length: 1.5 }]],
]) add(name, types, {}, 'malformed-limit');
for (const key of ['lookupTypes', 'extras', 'defaultDerives', 'extraDerives', 'meta',
  'camelCase', 'enumConversionError', 'forEachType', 'transparentBitflags', 'visibilityPrefix', 'typeMapping']) {
  add(`null-option-${key}`, alias, { [key]: null }, 'malformed-limit');
}
add('null-options', alias, null, 'malformed-limit');
for (const value of [null, true, false, {}, [], '', 'NaN', 'Infinity', '-Infinity',
  '1e400', '0x', '0b2', '12junk', '0xfffffffffffffffffffffffffffffffff',
  0, -1, 255, 256, 1.5, 9007199254740991, 9007199254740992, 1e40]) {
  const policy = typeof value === 'number' && Number.isInteger(value) && Math.abs(value) <= 9007199254740991
    ? 'exact' : 'numeric-limit';
  add(`enum-${JSON.stringify(value)}`, [{ name: 'Code', underlying: 'u8', variants: [['First', value]] }], {}, policy);
}
for (const token of ['1e400', '-1e400', 'NaN', 'Infinity']) {
  cases.push({ name: `raw-number-${token}`, policy: 'numeric-limit',
    input: `{"types":[{"name":"Code","underlying":"u8","variants":[["First",${token}]]}],"options":{}}` });
}
cases.push({ name: 'unpaired-surrogate', policy: 'malformed-limit',
  input: '{"types":[{"name":"Id","alias":"u8","description":"\\ud800"}],"options":{}}' });
for (const key of ['forEachType', 'typeMapping']) {
  add(`callback-string-${key}`, alias,
    { [key]: key === 'typeMapping' ? { Id: '() => "u64"' } : '(generated) => generated' }, 'callback-limit');
}
// Functions and inherited own-object defaults are not representable in JSON.
// Exercise the JS-only behavior explicitly, never pretend JSON transported them.
const jsOnly = [];
for (const [name, options] of [
  ['function-forEachType', { forEachType: ([generated]) => generated }],
  ['function-typeMapping', { typeMapping: { Id: () => 'u64' } }],
  ['inherited-extras', Object.assign(Object.create({ extras: ['// inherited'] }), {})],
  ['inherited-visibility', Object.create({ visibilityPrefix: 'pub(crate) ' })],
]) {
  try { jsOnly.push({ name, status: 'ok', output: generateString(structuredClone(alias), options) }); }
  catch (error) { jsOnly.push({ name, status: 'error', error: error.message }); }
}
const start = performance.now();
const results = [];
for (const [index, item] of cases.entries()) {
  const diagnostics = [];
  const originalError = console.error;
  const originalLog = console.log;
  let oracle;
  try {
    console.error = console.log = (...args) => diagnostics.push(args.map(String).join(' '));
    const request = JSON.parse(item.input);
    oracle = { status: 'ok', output: generateString(request.types, request.options) };
  } catch (error) { oracle = { status: 'error', error: error.message }; }
  finally { console.error = originalError; console.log = originalLog; }
  const rust = cp.spawnSync(binary, [], { input: item.input, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
  assert(!rust.error && rust.signal === null && rust.status !== null, rust.error?.message || `binary terminated: ${rust.signal}`);
  const actual = rust.status === 0 ? { status: 'ok', output: rust.stdout } : { status: 'error', error: rust.stderr };
  // Matching errors are only co-rejections, not exact diagnostic compatibility.
  const outcome = oracle.status === 'error' && actual.status === 'error' ? 'both-reject'
    : oracle.status !== actual.status ? 'acceptance-difference'
      : oracle.output === actual.output ? 'exact-bytes' : 'byte-difference';
  // An upstream throwing case cannot produce source bytes. Exact cases must
  // reproduce the diagnostic verbatim, apart from the CLI's fixed error prefix.
  // Merely co-rejecting is insufficient (and does not satisfy this gate).
  const exactDiagnostic = outcome === 'both-reject'
    && actual.error === `bendec2rs: ${oracle.error}\n`;
  const unexpected = item.policy === 'exact'
    && outcome !== 'exact-bytes' && !exactDiagnostic;
  const record = { ...item, outcome, exactDiagnostic, unexpected, diagnostics, oracle, rust: actual };
  fs.writeFileSync(path.join(receipts, `${String(index).padStart(3, '0')}.json`), JSON.stringify(record, null, 2) + '\n');
  results.push({ name: item.name, policy: item.policy, outcome, exactDiagnostic, unexpected,
    inputSha256: createHash('sha256').update(item.input).digest('hex') });
}
const summary = { oracle: 'bendec@0.52.0', cases: results.length,
  nodeVersion: process.version,
  binarySha256: createHash('sha256').update(fs.readFileSync(binary)).digest('hex'),
  unexpected: results.filter(r => r.unexpected).length,
  outcomes: Object.fromEntries(['exact-bytes', 'both-reject', 'acceptance-difference', 'byte-difference']
    .map(key => [key, results.filter(r => r.outcome === key).length])),
  harnessWallMsIncludingStartupIPC: performance.now() - start, results, jsOnly };
fs.writeFileSync(path.join(receipts, 'summary.json'), JSON.stringify(summary, null, 2) + '\n');
console.log(JSON.stringify({ ...summary, results: results.filter(r => r.unexpected), jsOnly: jsOnly.map(r => ({ name: r.name, status: r.status })) }, null, 2));
if (summary.unexpected) process.exitCode = 1;
