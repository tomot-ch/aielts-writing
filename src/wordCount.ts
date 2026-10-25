import type {TextRun} from './generated/dto';

/** Keep in sync with domain::word_count; shared examples live in contracts. */
export function wordCount(runs: TextRun[]): number {
  const text = runs.map(run => run.text).join('');
  return (text.match(/[A-Za-z0-9]+(?:['’][A-Za-z0-9]+)*|[\u4e00-\u9fff]/g) || []).length;
}
