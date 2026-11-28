// Parse, don't validate: a component's YAML body becomes a proven plain value or warnings naming field and line.
import { LineCounter, isMap, isScalar, isSeq, parseDocument } from 'yaml';

const INVALID = Symbol('invalid');
/** Every parsed mapping carries the plan line it was written on, for warnings raised after parsing. */
export const LINE = Symbol('line');

/** Field types. Each `check(node, at)` returns a value, `undefined` (absent optional), or INVALID after warning. */
export const t = {
  /** Text; a YAML number or boolean in a text field is its literal text. */
  text: (opts = {}) => ({ ...opts, describe: 'text', check: (node, at) => {
    if (!isScalar(node) || node.value === null || typeof node.value === 'object') return at.fail('must be text');
    const s = String(node.source ?? node.value).trim();
    return s ? s : at.fail('must not be empty');
  } }),
  num: (opts = {}) => ({ ...opts, describe: 'number', check: (node, at) => {
    if (!isScalar(node) || typeof node.value !== 'number' || !Number.isFinite(node.value)) return at.fail('must be a number');
    if (opts.integer && !Number.isInteger(node.value)) return at.fail('must be a whole number');
    if (opts.min !== undefined && node.value < opts.min) return at.fail(`must be ≥ ${opts.min}`);
    if (opts.max !== undefined && node.value > opts.max) return at.fail(`must be ≤ ${opts.max}`);
    return node.value;
  } }),
  bool: (opts = {}) => ({ ...opts, describe: 'true or false', check: (node, at) =>
    (isScalar(node) && typeof node.value === 'boolean' ? node.value : at.fail('must be true or false')) }),
  oneOf: (values, opts = {}) => ({ ...opts, describe: values.join(' | '), check: (node, at) => {
    const v = isScalar(node) ? String(node.value) : undefined;
    return values.includes(v) ? v : at.fail(`${isScalar(node) ? `"${node.value}"` : 'value'} is not one of ${values.join(', ')}`);
  } }),
  list: (item, opts = {}) => ({ ...opts, describe: `list of ${item.describe}`, check: (node, at) => {
    if (!isSeq(node)) return at.fail('must be a list');
    const items = node.items.map((n, i) => item.check(n, at.child(`[${i}]`, n))).filter((v) => v !== INVALID);
    const min = opts.min ?? 1;
    return items.length >= min ? items : at.fail(`needs at least ${min} valid item${min === 1 ? '' : 's'}`);
  } }),
  map: (fields) => ({ describe: 'mapping', fields, check: (node, at) => {
    if (!isMap(node)) return at.fail(`must be a mapping with ${Object.keys(fields).join(', ')}`);
    const given = new Map(node.items.map((p) => [String(isScalar(p.key) ? p.key.value : p.key), p]));
    [...given].filter(([k]) => !(k in fields)).forEach(([k, p]) =>
      at.child(`.${k}`, p.key).fail(`unknown field; expected ${Object.keys(fields).join(', ')}`));
    const out = {};
    let ok = true;
    Object.entries(fields).forEach(([k, spec]) => {
      const pair = given.get(k);
      const absent = !pair || pair.value === null || (isScalar(pair.value) && pair.value.value === null);
      if (absent) {
        if (spec.optional) out[k] = spec.default;
        else {
          at.child(`.${k}`, pair?.key ?? node).fail('is required');
          ok = false;
        }
        return;
      }
      const v = spec.check(pair.value, at.child(`.${k}`, pair.value));
      if (v === INVALID) {
        if (spec.optional) out[k] = spec.default;
        else ok = false;
      } else out[k] = v;
    });
    Object.defineProperty(out, LINE, { value: at.line() });
    return ok ? Object.freeze(out) : INVALID;
  } }),
  /** A mapping with exactly one of `fields`; normalised to `{ kind: <the key given>, value }`. */
  exactlyOne: (fields) => ({ describe: `one of ${Object.keys(fields).join(', ')}`, check: (node, at) => {
    const keys = Object.keys(fields);
    const fail = () => at.fail(`give exactly one of ${keys.join(', ')}`);
    if (!isMap(node) || node.items.length !== 1) return fail();
    const [pair] = node.items;
    const kind = isScalar(pair.key) ? String(pair.key.value) : undefined;
    if (!keys.includes(kind)) return fail();
    const value = fields[kind].check(pair.value, at.child(`.${kind}`, pair.value));
    return value === INVALID ? INVALID : Object.freeze({ kind, value });
  } }),
  /** Text written plainly, or a mapping with more fields; normalised to the mapping with `key` holding the text. */
  textOr: (key, fields) => {
    const asMap = t.map(fields);
    return { describe: `text or mapping`, check: (node, at) => {
      if (isMap(node)) return asMap.check(node, at);
      const s = t.text().check(node, at);
      if (s === INVALID) return s;
      const out = { ...Object.fromEntries(Object.entries(fields).map(([k, f]) => [k, f.default])), [key]: s };
      Object.defineProperty(out, LINE, { value: at.line() });
      return Object.freeze(out);
    } };
  },
};

/**
 * @param {string} name component name (the fence language)
 * @param {string} source fence body
 * @param {ReturnType<typeof t.map>} spec
 * @param {number} firstLine plan line of the body's first line
 * @returns {{ value: object | undefined, warnings: {text:string, line:number}[] }}
 */
export function parseComponent(name, source, spec, firstLine) {
  const lines = new LineCounter();
  const doc = parseDocument(source, { lineCounter: lines, uniqueKeys: true, prettyErrors: false });
  const warnings = [];
  const lineOf = (node) => firstLine + (node?.range ? lines.linePos(node.range[0]).line - 1 : 0);
  if (doc.errors.length) {
    const e = doc.errors[0];
    const line = firstLine + (e.linePos?.[0]?.line ?? 1) - 1;
    return { value: undefined, warnings: [{ text: `${name}: YAML error: ${e.message.split('\n')[0]}`, line }] };
  }
  const at = (path, node) => ({
    fail: (why) => {
      warnings.push({ text: `${name}${path}: ${why}`, line: lineOf(node) });
      return INVALID;
    },
    child: (seg, child) => at(`${path}${seg}`, child),
    line: () => lineOf(node),
  });
  const value = spec.check(doc.contents, at('', doc.contents));
  return { value: value === INVALID ? undefined : value, warnings };
}
