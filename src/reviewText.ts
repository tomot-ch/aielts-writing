import type {ReviewAnnotation, TextRun} from './generated/dto';

export type DiffPart = {text: string; changed: boolean; start: number; end: number};
// Preserve every character, including punctuation and whitespace. Bound LCS work for long excerpts.
const tokens = (text: string) => text.match(/[\p{L}\p{N}_]+|\s+|[^\p{L}\p{N}_\s]/gu) || [];
export function textDiff(original: string, suggestion: string): {before: DiffPart[]; after: DiffPart[]} {
  const a = tokens(original), b = tokens(suggestion);
  const sameA = new Set<number>(), sameB = new Set<number>();
  if (a.length * b.length <= 250_000) {
    const width = b.length + 1, grid = new Uint32Array((a.length + 1) * width);
    for (let i = a.length - 1; i >= 0; i--) for (let j = b.length - 1; j >= 0; j--)
      grid[i * width + j] = a[i] === b[j] ? grid[(i + 1) * width + j + 1] + 1 : Math.max(grid[(i + 1) * width + j], grid[i * width + j + 1]);
    let i = 0, j = 0;
    while (i < a.length && j < b.length) {
      if (a[i] === b[j]) {sameA.add(i++); sameB.add(j++);}
      else if (grid[(i + 1) * width + j] >= grid[i * width + j + 1]) i++;
      else j++;
    }
  } else {
    let i = 0;
    while (i < Math.min(a.length, b.length) && a[i] === b[i]) {sameA.add(i); sameB.add(i++);}
    let x = a.length - 1, y = b.length - 1;
    while (x >= i && y >= i && a[x] === b[y]) {sameA.add(x--); sameB.add(y--);}
  }
  const parts = (list: string[], same: Set<number>) => {
    const result: DiffPart[] = [];
    let offset = 0;
    list.forEach((text, i) => {
      const changed = !same.has(i), last = result[result.length - 1];
      if (last?.changed === changed) {last.text += text; last.end += text.length;}
      else result.push({text, changed, start: offset, end: offset + text.length});
      offset += text.length;
    });
    return result;
  };
  return {before: parts(a, sameA), after: parts(b, sameB)};
}

export type SourcePart = {text: string; start: number; color: string; ids: string[]; selected: boolean; deleted: boolean};
export function sourceParts(runs: TextRun[], annotations: ReviewAnnotation[], selectedId: string): SourcePart[] {
  const text = runs.map(r => r.text).join('');
  const located = annotations.filter((a): a is ReviewAnnotation & {start: number; end: number} => a.start !== null && a.end !== null && a.start >= 0 && a.end > a.start && a.end <= text.length);
  const selected = located.find(a => a.id === selectedId);
  const deletions = selected?.scope === 'text' && selected.suggestion !== null
    ? textDiff(selected.original, selected.suggestion).before.filter(p => p.changed).map(p => ({start: selected.start + p.start, end: selected.start + p.end})) : [];
  const boundaries = new Set([0, text.length]);
  let offset = 0;
  const colors = runs.map(r => {const start = offset; offset += r.text.length; boundaries.add(offset); return {...r, start, end: offset};});
  for (const range of [...located, ...deletions]) {boundaries.add(range.start); boundaries.add(range.end);}
  const points = [...boundaries].sort((a, b) => a - b);
  return points.slice(0, -1).map((start, i) => ({
    text: text.slice(start, points[i + 1]), start,
    color: colors.find(c => start >= c.start && start < c.end)?.color || 'black',
    ids: located.filter(a => start >= a.start && start < a.end).map(a => a.id),
    selected: !!selected && start >= selected.start && start < selected.end,
    deleted: deletions.some(d => start >= d.start && start < d.end),
  }));
}
