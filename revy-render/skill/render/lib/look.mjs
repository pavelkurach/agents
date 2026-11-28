// Pure: the optional look file, parsed into a proven shape, and its ordering rule.
export class LookError extends Error {}

const strings = (value, key) => {
  if (value === undefined) return [];
  if (!Array.isArray(value) || !value.every((v) => typeof v === 'string')) {
    throw new LookError(`look.${key} must be an array of strings`);
  }
  return value;
};

/**
 * @returns {{ order: string[], fold: string[], emphasis: string[], collapse: string[], tail: number | null, watermark: string | null }}
 */
export function parseLook(raw) {
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) throw new LookError('look must be a JSON object');
  const { watermark } = raw;
  if (watermark !== undefined && watermark !== null && typeof watermark !== 'string') {
    throw new LookError('look.watermark must be a string or null');
  }
  const { tail } = raw;
  if (tail !== undefined && tail !== null && !(Number.isInteger(tail) && tail >= 1)) {
    throw new LookError('look.tail must be a whole number ≥ 1, or null');
  }
  return {
    order: strings(raw.order, 'order'),
    fold: strings(raw.fold, 'fold'),
    emphasis: strings(raw.emphasis, 'emphasis'),
    collapse: strings(raw.collapse, 'collapse'),
    tail: tail ?? null,
    watermark: watermark ?? null,
  };
}

/** Listed titles first, in `order`; the rest keep their relative position after them. */
export function orderSections(sections, order) {
  const rank = (s) => {
    const i = order.indexOf(s.title);
    return i === -1 ? order.length : i;
  };
  return sections
    .map((s, i) => ({ s, i }))
    .sort((a, b) => rank(a.s) - rank(b.s) || a.i - b.i)
    .map(({ s }) => s);
}
