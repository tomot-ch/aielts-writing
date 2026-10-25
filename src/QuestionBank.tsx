import {useEffect, useRef, useState} from 'react';
import {ArrowRight, BookOpen, Check, Clock3, ImageOff, Maximize2, Minimize2, Search, Trash2, Upload, X} from 'lucide-react';
import {api, type Question} from './api';

type Props = {
  questions: Question[];
  selected: Question | null;
  image: string;
  error: string;
  busy: boolean;
  onSelect: (question: Question) => void;
  onImport: () => void;
  onDelete: (question: Question) => void;
  onWrite: (question: Question) => void;
};

export function QuestionBank(props: Props) {
  const {questions, selected, onSelect, onImport, busy} = props;
  const [query, setQuery] = useState('');
  const [task, setTask] = useState<'all' | Question['taskType']>('all');
  const [previewOpen, setPreviewOpen] = useState(false);
  const search = query.trim().toLocaleLowerCase();
  const filtered = questions.filter(q => (task === 'all' || q.taskType === task) && q.title.toLocaleLowerCase().includes(search));

  return <div className="page-wrap oil-page oil-bank">
    <div className="page-heading oil-heading"><div><h1>从题库选择题目</h1></div><button className="primary" disabled={busy} onClick={onImport}><Upload size={16}/>导入题目</button></div>
    <div className="oil-library-tools">
      <div className="oil-search"><Search size={17} aria-hidden="true"/><input aria-label="搜索题目名称" placeholder="搜索题目名称…" value={query} onChange={e => setQuery(e.target.value)}/>{query && <button className="icon-button" aria-label="清空搜索" onClick={() => setQuery('')}><X size={14}/></button>}</div>
      <div className="oil-task-filters" role="group" aria-label="按 Task 筛选">{(['all', 'task1', 'task2'] as const).map(value => <button key={value} aria-pressed={task === value} onClick={() => setTask(value)}>{value === 'all' ? '全部' : value === 'task1' ? 'Task 1' : 'Task 2'}</button>)}</div>
      <p className="oil-result-count" role="status">{task === 'all' && !search ? `共 ${questions.length} 道题` : `${filtered.length} / ${questions.length} 道题`}</p>
    </div>
    <div className="oil-question-gallery" aria-label="题库画廊">
      {filtered.map(q => <button key={q.id} className={`oil-question-row ${selected?.id === q.id ? 'chosen' : ''}`} aria-label={`预览题目：${q.title}`} aria-pressed={selected?.id === q.id} onClick={() => {onSelect(q); setPreviewOpen(true);}}>
        <QuestionThumbnail question={q}/>
        <span className="oil-question-card-footer"><span className="oil-question-copy"><small>{q.taskType === 'task1' ? 'Task 1 · 20 分钟' : 'Task 2 · 40 分钟'}{q.imageCount > 1 && ` · ${q.imageCount} 张图片`}</small><strong><QuestionTitle question={q}/></strong></span>{selected?.id === q.id ? <Check size={17}/> : <ArrowRight size={17}/>}</span>
      </button>)}
    </div>
    {!filtered.length && <div className="oil-list-empty"><BookOpen size={30} strokeWidth={1.5}/><h2>{questions.length ? '没有找到题目' : '题库里还没有题目'}</h2><p>{questions.length ? '试试其他名称，或清除筛选。' : '导入题目图片后，就可以开始练习。'}</p><button className="secondary" onClick={() => {if (questions.length) {setQuery(''); setTask('all');} else onImport();}}>{questions.length ? '清除筛选' : '导入题目'}</button></div>}
    {previewOpen && selected && <QuestionPreview key={selected.id} {...props} selected={selected} onClose={() => setPreviewOpen(false)}/>}
  </div>;
}

function QuestionThumbnail({question}: {question: Question}) {
  const ref = useRef<HTMLSpanElement>(null);
  const [image, setImage] = useState(question.imageFile.startsWith('data:image/') ? question.imageFile : '');
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (image || failed) return;
    let alive = true;
    const load = () => {void api.call('question_image', {questionId: question.id}).then(value => {if (alive) setImage(value);}).catch(() => {if (alive) setFailed(true);});};
    if (typeof IntersectionObserver === 'undefined') {load(); return () => {alive = false;};}
    const observer = new IntersectionObserver(entries => {if (entries.some(entry => entry.isIntersecting)) {observer.disconnect(); load();}}, {rootMargin: '200px'});
    if (ref.current) observer.observe(ref.current);
    return () => {alive = false; observer.disconnect();};
  }, [question.id, image, failed]);
  return <span ref={ref} className="oil-question-thumbnail">{image && !failed ? <img src={image} alt="" loading="lazy" onError={() => setFailed(true)}/> : <span className="oil-thumbnail-placeholder">{failed ? <ImageOff size={24}/> : <BookOpen size={24} strokeWidth={1.5}/>}<span>{failed ? '预览暂不可用' : '正在读取原图…'}</span></span>}</span>;
}

function QuestionPreview({selected, image, error, busy, onSelect, onDelete, onWrite, onClose}: Props & {selected: Question; onClose: () => void}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const previous = document.activeElement;
    const node = ref.current!;
    node.showModal();
    return () => {node.close(); if (previous instanceof HTMLElement && previous.isConnected) previous.focus();};
  }, []);
  return <dialog ref={ref} className="oil-question-sheet" aria-labelledby="oil-preview-title" onCancel={e => {e.preventDefault(); if (!busy) onClose();}}>
    <header className="oil-preview-heading"><div><h2 id="oil-preview-title"><QuestionTitle question={selected}/></h2><p>{selected.taskType === 'task1' ? 'Task 1' : 'Task 2'} · 原题图片</p></div><div><button className="delete-action" aria-label={`删除题目：${selected.title}`} title="删除题目" disabled={busy} onClick={() => onDelete(selected)}><Trash2 size={17}/></button><button className="icon-button" aria-label="关闭题目预览" disabled={busy} onClick={onClose} autoFocus><X size={20}/></button></div></header>
    <div className="oil-paper-canvas">{error ? <div className="oil-preview-empty" role="alert"><ImageOff size={26}/><strong>暂时无法读取题目图片</strong><p>{error}</p><button className="secondary" onClick={() => onSelect(selected)}>重新读取</button></div> : image ? <QuestionImage key={selected.id} image={image} title={selected.title} onRetry={() => onSelect(selected)}/> : <div className="oil-preview-empty" role="status">正在读取原题图片…</div>}</div>
    <footer className="oil-preview-footer"><div><span><Clock3 size={15}/>{selected.taskType === 'task1' ? '20 分钟 · 至少 150 词' : '40 分钟 · 至少 250 词'}</span><small>计时默认关闭</small></div><button className="primary" disabled={busy} onClick={() => onWrite(selected)}>{busy ? '正在打开…' : '用此题写作'}<ArrowRight size={16}/></button></footer>
  </dialog>;
}

function QuestionImage({image, title, onRetry}: {image: string; title: string; onRetry: () => void}) {
  const [failed, setFailed] = useState(false);
  const [zoomed, setZoomed] = useState(false);
  return failed ? <div className="oil-preview-empty" role="alert"><ImageOff size={26}/><strong>题目图片未能显示</strong><button className="secondary" onClick={onRetry}>重新读取</button></div> : <div className="oil-image-view"><div className="oil-image-toolbar"><button className="secondary" aria-pressed={zoomed} onClick={() => setZoomed(value => !value)}>{zoomed ? <Minimize2 size={14}/> : <Maximize2 size={14}/>} {zoomed ? '完整预览' : '放大原图'}</button></div><div className={`oil-image-scroll ${zoomed ? 'zoomed' : ''}`} tabIndex={zoomed ? 0 : undefined} aria-label={zoomed ? '放大的题目原图，可横向和纵向滚动' : undefined}><img src={image} alt={title} onError={() => setFailed(true)}/></div></div>;
}

function QuestionTitle({question}: {question: Question}) {
  const title = question.title.replace(/\s*(?:[·|:—–-]\s*)?Task\s*[12]\s*$/i, '').trim() || question.title;
  return title.split(/(Test\s*\d+|Task\s*[12])/gi).map((part, index) => <span className="oil-title-unit" key={index}>{part}</span>);
}
