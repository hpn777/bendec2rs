// Development only: resolve an explicitly supplied installation or npm ci's pin.
const path = require('node:path');
const assert = require('node:assert/strict');
const oracleRoot = process.env.BENDEC_ORACLE_ROOT
  ? path.resolve(process.env.BENDEC_ORACLE_ROOT)
  : path.dirname(require.resolve('bendec/package.json'));
assert.equal(require(path.join(oracleRoot, 'package.json')).version, '0.52.0');
module.exports = require(path.join(oracleRoot, 'dist/tools/rsGenerator'));
