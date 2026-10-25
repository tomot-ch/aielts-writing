import {useEffect, useMemo, useRef, useState} from 'react';
import {ChevronDown, Copy, FileCheck2} from 'lucide-react';
import Markdown from 'react-markdown';
import {api} from './api';
import type {AnalysisRecord, ReviewAnnotation} from './generated/dto';
import {sourceParts, textDiff} from './reviewText';
import {wordCount} from './wordCount';
import './annotated-review.css';

const kindLabel: Record<string, string> = {error: '确定错误', improvement: '可优化', verify: '待核实'};
const criteria: Record<string, string> = {TA: '任务完成度', TR: '任务回应', CC: '连贯与衔接', LR: '词汇资源', GRA: '语法多样性与准确性', overall: '综合预估'};

function DiffText({original, suggestion, side}: {original: string; suggestion: string; side: 'before' | 'after'}) {
  const parts = useMemo(() => textDiff(original, suggestion)[side], [original, suggestion, side]);
  return <>{parts.map((p, i) => p.changed ? side === 'before'
    ? <del key={i}>{p.text}</del> : <ins key={i}>{p.text}</ins> : <span key={i}>{p.text}</span>)}</>;
}

function ReviewCard({note, index, selected, onSelect}: {note: ReviewAnnotation; index: number; selected: boolean; onSelect: () => void}) {
  return <article className={`review-card ${selected ? 'is-selected' : ''}`} data-note-id={note.id}>
    <button type="button" className="review-card-heading" aria-pressed={selected} onClick={onSelect}>
      <span className="review-note-number">{String(index + 1).padStart(2, '0')}</span>
      <strong>{note.issueType}</strong><span className={`review-kind ${note.kind}`}>{kindLabel[note.kind]}</span>
      <span className="review-locate-label">{selected ? '已选中' : note.locationNote ? '查看建议' : '定位原文'}</span>
    </button>
    <div className="review-card-body" onClick={onSelect}>
      {note.original && <div className="review-sentence"><small>原文</small><div>{note.suggestion !== null
        ? <DiffText original={note.original} suggestion={note.suggestion} side="before"/> : note.original}</div></div>}
      {note.suggestion !== null && <div className="review-sentence"><small>修改后</small><div>{note.suggestion === '' ? <em>删除此处</em> : <DiffText original={note.original} suggestion={note.suggestion} side="after"/>}</div></div>}
      {note.scope !== 'text' && <span className="review-scope">{note.scope === 'essay' ? '全文建议' : '段落建议'}</span>}
      <p className="review-explanation">{note.explanation}</p>
      {note.locationNote && <p className="review-location-note">无法定位：{note.locationNote}。请对照原文查看。</p>}
    </div>
  </article>;
}

export function AnnotatedReview({record, versionName, hasQuestion, notice}: {record: AnalysisRecord; versionName: string; hasQuestion: boolean; notice: (s: string) => void}) {
  const review = record.review;
  const [selectedId, setSelectedId] = useState('');
  const [relatedIds, setRelatedIds] = useState<string[]>([]);
  const [questionOpen, setQuestionOpen] = useState(false);
  const [question, setQuestion] = useState('');
  const [questionError, setQuestionError] = useState('');
  const source = useRef<HTMLDivElement>(null), cards = useRef<HTMLDivElement>(null);
  const parts = useMemo(() => review ? sourceParts(review.runs, review.annotations, selectedId) : [], [review, selectedId]);
  const [scrollRequest, setScrollRequest] = useState<{id: string; side: 'source' | 'cards'} | null>(null);
  useEffect(() => {
    if (!scrollRequest) return;
    const pane = scrollRequest.side === 'source' ? source.current : cards.current;
    const target = scrollRequest.side === 'source'
      ? pane?.querySelector<HTMLElement>('[data-selected-source="true"]')
      : Array.from(pane?.querySelectorAll<HTMLElement>('[data-note-id]') || []).find(el => el.dataset.noteId === scrollRequest.id);
    if (!pane || !target) return;
    const p = pane.getBoundingClientRect(), t = target.getBoundingClientRect();
    if (t.top < p.top + 12 || t.bottom > p.bottom - 12) pane.scrollTo({top: pane.scrollTop + t.top - p.top - 24, behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'instant' : 'smooth'});
  }, [scrollRequest]);
  useEffect(() => {
    if (!questionOpen || !hasQuestion || !review) return;
    let alive = true;
    setQuestion(''); setQuestionError('');
    void api.call('session_question_image', {sessionId: review.source.sessionId})
      .then(image => {if (alive) setQuestion(image);}).catch(e => {if (alive) setQuestionError(String(e));});
    return () => {alive = false;};
  }, [questionOpen, hasQuestion, review?.source.sessionId]);
  const select = (id: string, side: 'source' | 'cards', related?: string[]) => {
    setSelectedId(id);
    setRelatedIds(related || []);
    setScrollRequest({id, side});
  };
  if (!review) return <div className="review-legacy"><FileCheck2 size={24}/><h2>这份历史报告没有结构化批注</h2><p>原报告仍可阅读、追问和导出。新建对话后可生成批注式批改。</p><Markdown skipHtml>{record.markdown}</Markdown></div>;
  return <div className="annotated-review" aria-label="批注式批改">
    <section className="review-source-pane" aria-label="固定版本原文">
      <div className="review-pane-heading"><div><strong>送审原文</strong><small>{versionName} · 只读</small></div><button className="icon-button" title="复制固定原文" aria-label="复制固定原文" onClick={() => void navigator.clipboard.writeText(review.runs.map(r => r.text).join('')).then(() => notice('已复制固定原文')).catch(() => notice('复制失败，请手动选择原文复制。'))}><Copy size={16}/></button></div>
      {hasQuestion && <div className="review-question"><button className="review-question-toggle" aria-expanded={questionOpen} onClick={() => setQuestionOpen(v => !v)}>题目原图 <span>{questionOpen ? '收起' : '展开'}</span><ChevronDown size={15}/></button>{questionOpen && <div className="review-question-image">{question ? <img src={question} alt="本篇送审题目原图"/> : <p>{questionError || '正在加载题目…'}</p>}</div>}</div>}
      <div className="review-source-meta"><span>{wordCount(review.runs)} words</span><span>点击标记文字查看建议</span></div>
      {relatedIds.length > 1 && <nav className="review-related" aria-label="此处相关批注"><span>此处 {relatedIds.length} 条建议</span>{relatedIds.map(id => {const n = review.annotations.findIndex(a => a.id === id); return <button key={id} aria-pressed={selectedId === id} onClick={() => select(id, 'cards', relatedIds)}>{n + 1}</button>;})}</nav>}
      <div className="review-source-scroll" ref={source} tabIndex={0} aria-label="原文滚动区"><div className="review-source-text">{parts.map(p => <span key={p.start} className={`ink-${p.color} ${p.ids.length ? 'has-annotation' : ''} ${p.selected ? 'selected-source' : ''} ${p.deleted ? 'source-deletion' : ''}`}
        data-selected-source={p.selected || undefined} data-source-start={p.start} role={p.ids.length ? 'button' : undefined} tabIndex={p.ids.length ? 0 : undefined}
        aria-label={p.ids.length ? `${p.text}，${p.ids.length} 条相关批注` : undefined}
        onClick={p.ids.length ? () => select(p.ids[(p.ids.indexOf(selectedId) + 1) % p.ids.length], 'cards', p.ids) : undefined}
        onKeyDown={p.ids.length ? e => {if (e.key === 'Enter' || e.key === ' ') {e.preventDefault(); select(p.ids[(p.ids.indexOf(selectedId) + 1) % p.ids.length], 'cards', p.ids);}} : undefined}>{p.text}</span>)}</div></div>
      <div className="review-source-footer">固定版本 {review.source.versionId.slice(-8)} · 笔迹颜色保留</div>
    </section>
    <section className="review-results-pane" aria-label="批改建议">
      <div className="review-pane-heading"><div><strong>批改建议</strong><small>{review.annotations.length} 条 · {record.modelName}</small></div><span className="review-legend"><del>删改</del><ins>新增</ins></span></div>
      <div className="review-results-scroll" ref={cards} tabIndex={0} aria-label="批改滚动区">
        {review.warnings.length > 0 && <div className="review-warning" role="status">{review.warnings.map((w,i) => <p key={i}>{w}</p>)}<details><summary>查看收到的原始回复</summary><pre>{review.rawResponse}</pre></details></div>}
        {review.summary && <p className="review-summary">{review.summary}</p>}
        {review.scores.length > 0 && <details className="review-scores" open><summary>评分概览 <span>教学预估</span></summary><div>{review.scores.map(s => <div key={s.criterion}><strong>{s.band || '—'}</strong><span>{criteria[s.criterion] || s.criterion}</span><p>{s.explanation}</p></div>)}</div></details>}
        {review.annotations.length ? <div className="review-card-list">{review.annotations.map((note, i) => <ReviewCard key={note.id} note={note} index={i} selected={note.id === selectedId} onSelect={() => select(note.id, 'source')}/>)}</div> : <div className="review-empty"><FileCheck2 size={24}/><p>{review.warnings.length ? '本次没有可显示的结构化批注，请查看原始回复或切换对话。' : '本次没有提出局部修改建议。可切换对话继续追问。'}</p></div>}
        {review.issueSummary && <details className="review-issue-summary"><summary>问题归纳与下次自查</summary><p>{review.issueSummary}</p></details>}
      </div>
    </section>
  </div>;
}
