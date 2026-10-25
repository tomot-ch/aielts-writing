import type {InkColor,TextRun} from './api';
export {wordCount} from './wordCount';
export function renderRuns(el:HTMLElement,runs:TextRun[]){el.replaceChildren();for(const run of runs){const span=document.createElement('span');span.dataset.ink=run.color;span.className=`ink-${run.color}`;span.textContent=run.text;el.append(span)}}
export function readRuns(el:HTMLElement):TextRun[]{
 const out:TextRun[]=[];
 const add=(text:string,color:InkColor)=>{if(!text)return;const last=out[out.length-1];if(last?.color===color)last.text+=text;else out.push({text,color})};
 const walk=(node:Node,color:InkColor)=>{
  if(node.nodeType===Node.TEXT_NODE){add(node.textContent||'',color);return}
  if(!(node instanceof HTMLElement))return;
  if(node.tagName==='BR'){add('\n',color);return}
  const ink=(node.dataset.ink as InkColor)||color;
  const block=['DIV','P'].includes(node.tagName)&&node!==el;
  if(block&&out.length&&!(out[out.length-1].text.endsWith('\n')))add('\n',ink);
  for(const child of node.childNodes)walk(child,ink);
 };
 for(const child of el.childNodes)walk(child,'black');return out;
}
export function insertColored(el:HTMLElement,text:string,color:InkColor){
 el.focus();const selection=window.getSelection();if(!selection)return;let range=selection.rangeCount?selection.getRangeAt(0):document.createRange();if(!el.contains(range.commonAncestorContainer)){range=document.createRange();range.selectNodeContents(el);range.collapse(false)}
 range.deleteContents();const span=document.createElement('span');span.dataset.ink=color;span.className=`ink-${color}`;span.textContent=text;range.insertNode(span);range.setStartAfter(span);range.collapse(true);selection.removeAllRanges();selection.addRange(range);
}
