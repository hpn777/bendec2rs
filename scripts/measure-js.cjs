// Compare this warmed generation time with examples/measure.rs, not CLI wall time.
const fs = require('node:fs');
const assert = require('node:assert/strict');
const { performance } = require('node:perf_hooks');
const { generateString } = require('./oracle.cjs');
const bytes = fs.readFileSync(process.argv[2], 'utf8');
let start = performance.now();
const corpus = JSON.parse(bytes);
const parseUs = (performance.now() - start) * 1000;
const iterations = Number(process.argv[3] || 100);
assert(Number.isSafeInteger(iterations) && iterations > 0);
// Restore the one declarative mapping fixture to its equivalent JS callbacks.
for (const request of corpus) {
  if (request.options.typeMapping) {
    request.options.typeMapping = Object.fromEntries(Object.entries(request.options.typeMapping)
      .map(([key, template]) => [key, size => template.replace(/{{\s*size\s*}}/g, size)]));
  }
}
function render() {
  let total = 0;
  for (const request of corpus) total += Buffer.byteLength(generateString(request.types, request.options));
  return total;
}
// Missing external field lookups only print diagnostics in upstream; retain
// their counts but exclude terminal I/O from an emitter-only comparison.
const log = console.log;
let diagnostics = 0;
console.log = () => { diagnostics++; };
let expected, elapsed;
try {
  expected = render();
  start = performance.now();
  for (let i = 0; i < iterations; i++) assert.equal(render(), expected);
  elapsed = performance.now() - start;
} finally { console.log = log; }
console.log(JSON.stringify({ parse_us: parseUs, generation_us: elapsed * 1000, iterations, bytes_per_iteration: expected, diagnostic_messages_counted_not_printed: diagnostics }));
