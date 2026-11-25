import type {InkColor, TextRun} from './api';

type SelectionOffsets = {start: number; end: number};

// WebKit can replace an inserted span with inline colour when it edits across
// formatting boundaries. Decode the same palette used by the editor styles.
const inlineInk = new Map<string, InkColor>([
  ['rgb(32, 33, 36)', 'black'],
  ['rgb(233, 234, 237)', 'black'],
  ['rgb(243, 243, 240)', 'black'],
  ['rgb(37, 99, 235)', 'blue'],
  ['rgb(21, 128, 61)', 'green'],
  ['rgb(220, 38, 38)', 'red'],
]);

function inkSpan(text: string, color: InkColor): HTMLSpanElement {
  const span = document.createElement('span');
  span.dataset.ink = color;
  span.className = `ink-${color}`;
  span.textContent = text;
  return span;
}

function trailingCaret(): HTMLBRElement {
  const br = document.createElement('br');
  br.setAttribute('data-editor-caret', '');
  return br;
}

export function renderRuns(el: HTMLElement, runs: TextRun[]) {
  el.replaceChildren(...runs.map(({text, color}) => inkSpan(text, color)));
  // The browser needs a position on the empty line after a final newline.
  if (el.textContent?.endsWith('\n')) el.append(trailingCaret());
}

function addRun(runs: TextRun[], text: string, color: InkColor) {
  if (!text) return;
  const last = runs[runs.length - 1];
  if (last?.color === color) last.text += text;
  else runs.push({text, color});
}

export function readRuns(el: HTMLElement): TextRun[] {
  // Deleting the entire document can leave WebKit's empty-line BR placeholders.
  if (!el.textContent) return [];
  const runs: TextRun[] = [];
  const walk = (node: Node, inherited: InkColor) => {
    if (node.nodeType === Node.TEXT_NODE) {
      addRun(runs, node.textContent || '', inherited);
      return;
    }
    if (!(node instanceof HTMLElement) || node.hasAttribute('data-editor-caret')) return;
    if (node.tagName === 'BR') {
      addRun(runs, '\n', inherited);
      return;
    }
    const color = inlineInk.get(node.style.color) ?? (node.dataset.ink as InkColor | undefined) ?? inherited;
    const last = runs[runs.length - 1];
    if ((node.tagName === 'DIV' || node.tagName === 'P') && last && !last.text.endsWith('\n')) {
      addRun(runs, '\n', color);
    }
    for (const child of node.childNodes) walk(child, color);
  };
  for (const child of el.childNodes) walk(child, 'black');
  return runs;
}

function rangeInEditor(el: HTMLElement): Range | null {
  const selection = window.getSelection();
  if (!selection?.rangeCount) return null;
  const range = selection.getRangeAt(0);
  return el.contains(range.commonAncestorContainer) ? range : null;
}

function insertColored(el: HTMLElement, text: string, color: InkColor) {
  el.focus();
  const selection = window.getSelection();
  if (!selection) return;
  let range = rangeInEditor(el);
  if (!range) {
    range = document.createRange();
    range.selectNodeContents(el);
    range.collapse(false);
  }
  const span = inkSpan(text, color);
  const suffix = range.cloneRange();
  suffix.setStart(range.endContainer, range.endOffset);
  suffix.setEnd(el, el.childNodes.length);
  const caret = text.endsWith('\n') && !suffix.toString() ? trailingCaret() : null;
  selection.removeAllRanges();
  selection.addRange(range);

  // Editing through the browser keeps typing and pasting in its undo history.
  // The HTML is generated from DOM nodes with textContent, including pasted text.
  if (document.execCommand('insertHTML', false, span.outerHTML + (caret?.outerHTML ?? ''))) return;

  range.deleteContents();
  range.insertNode(span);
  if (caret && !el.querySelector('[data-editor-caret]')) span.after(caret);
  range.setStartAfter(span);
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
}

function selectionOffsets(el: HTMLElement, range: AbstractRange | null = rangeInEditor(el)): SelectionOffsets | null {
  if (!range || !el.contains(range.startContainer) || !el.contains(range.endContainer)) return null;
  const lengthBefore = (container: Node, offset: number) => {
    const prefix = document.createRange();
    prefix.selectNodeContents(el);
    prefix.setEnd(container, offset);
    const fragment = document.createElement('div');
    fragment.append(prefix.cloneContents());
    return readRuns(fragment).reduce((length, run) => length + run.text.length, 0);
  };
  return {start: lengthBefore(range.startContainer, range.startOffset), end: lengthBefore(range.endContainer, range.endOffset)};
}

function restoreSelection(el: HTMLElement, offsets: SelectionOffsets | null) {
  if (!offsets) return;
  const range = document.createRange();
  const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
  const point = (offset: number): [Node, number] => {
    walker.currentNode = el;
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const length = node.textContent?.length ?? 0;
      if (offset <= length) return [node, offset];
      offset -= length;
    }
    return [el, el.childNodes.length];
  };
  range.setStart(...point(offsets.start));
  range.setEnd(...point(offsets.end));
  const selection = window.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(range);
}

const runText = (runs: TextRun[]) => runs.map(run => run.text).join('');

// IMEs manage their own provisional text. Colour the committed replacement and
// reconstruct the unchanged parts from the pre-composition runs.
export function colorCommittedInput(before: TextRun[], after: TextRun[], color: InkColor, selection: SelectionOffsets | null): TextRun[] {
  const original = runText(before);
  const current = runText(after);
  let start = selection?.start ?? 0;
  let end = selection?.end ?? 0;
  if (!selection || current.length < start + original.length - end ||
      !current.startsWith(original.slice(0, start)) || !current.endsWith(original.slice(end))) {
    start = 0;
    while (start < original.length && start < current.length && original[start] === current[start]) start++;
    let suffix = 0;
    while (suffix < original.length - start && suffix < current.length - start &&
           original[original.length - 1 - suffix] === current[current.length - 1 - suffix]) suffix++;
    end = original.length - suffix;
  }
  // Never slice existing text between the two UTF-16 units of a character.
  const insideSurrogate = (text: string, offset: number) => /[\uDC00-\uDFFF]/.test(text[offset] ?? '');
  if (start > 0 && (insideSurrogate(original, start) || insideSurrogate(current, start))) start--;
  if (insideSurrogate(original, end)) end++;

  const result: TextRun[] = [];
  let offset = 0;
  for (const run of before) {
    addRun(result, run.text.slice(0, Math.max(0, start - offset)), run.color);
    offset += run.text.length;
  }
  addRun(result, current.slice(start, current.length - (original.length - end)), color);
  offset = 0;
  for (const run of before) {
    addRun(result, run.text.slice(Math.max(0, end - offset)), run.color);
    offset += run.text.length;
  }
  return result;
}

type InputOptions = {color: () => InkColor; canEdit: () => boolean; onChange: () => void};
type InputSnapshot = {runs: TextRun[]; selection: SelectionOffsets | null; color: InkColor};

export function bindColoredInput(el: HTMLElement, options: InputOptions) {
  let composition: InputSnapshot | null = null;
  let nativeInsertion: InputSnapshot | null = null;
  let inserting = false;
  const snapshot = (range?: AbstractRange) => ({runs: readRuns(el), selection: selectionOffsets(el, range), color: options.color()});
  const reconcile = (pending: InputSnapshot, confirmed: boolean) => {
    const after = readRuns(el);
    if (!confirmed && runText(after) === runText(pending.runs)) return;
    const colored = colorCommittedInput(pending.runs, after, pending.color, pending.selection);
    if (colored.length === after.length && colored.every((run, i) => run.text === after[i].text && run.color === after[i].color)) return;
    const caret = selectionOffsets(el);
    renderRuns(el, colored);
    restoreSelection(el, caret);
  };
  const insert = (text: string) => {
    inserting = true;
    try { insertColored(el, text.replace(/\r\n?/g, '\n'), options.color()); }
    finally { inserting = false; }
    options.onChange();
  };
  const beforeInput = (event: InputEvent) => {
    // React's onBeforeInput is a polyfill and does not expose native inputType.
    if (inserting || composition || event.isComposing) return;
    nativeInsertion = null;
    if (!options.canEdit()) {
      if (event.cancelable) event.preventDefault();
      return;
    }
    const lineBreak = event.inputType === 'insertParagraph' || event.inputType === 'insertLineBreak';
    const textInput = ['insertText', 'insertReplacementText', 'insertFromComposition'].includes(event.inputType);
    if (!lineBreak && !textInput) return;
    const text = lineBreak ? '\n' : event.data ?? event.dataTransfer?.getData('text/plain');
    const target = event.getTargetRanges?.()[0];
    // Reconcile after the browser commits text if it owns the replacement
    // range, omits the payload, or will not let us cancel the insertion.
    if (!event.cancelable || !text || (target && event.inputType !== 'insertText' && !lineBreak)) {
      nativeInsertion = snapshot(target);
      return;
    }
    event.preventDefault();
    insert(text);
  };
  const paste = (event: ClipboardEvent) => {
    event.preventDefault();
    const text = event.clipboardData?.getData('text/plain');
    if (text && options.canEdit()) insert(text);
  };
  const compositionStart = () => {
    nativeInsertion = null;
    if (options.canEdit()) composition = snapshot();
  };
  const compositionEnd = (event: CompositionEvent) => {
    const pending = composition;
    composition = null;
    if (!pending) return;
    reconcile(pending, !!event.data);
    options.onChange();
  };
  const input = () => {
    if (inserting || composition) return;
    if (nativeInsertion) {
      reconcile(nativeInsertion, true);
      nativeInsertion = null;
    }
    options.onChange();
  };
  el.addEventListener('beforeinput', beforeInput);
  el.addEventListener('paste', paste);
  el.addEventListener('compositionstart', compositionStart);
  el.addEventListener('compositionend', compositionEnd);
  el.addEventListener('input', input);
  return () => {
    el.removeEventListener('beforeinput', beforeInput);
    el.removeEventListener('paste', paste);
    el.removeEventListener('compositionstart', compositionStart);
    el.removeEventListener('compositionend', compositionEnd);
    el.removeEventListener('input', input);
  };
}
