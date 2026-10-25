import {forwardRef, useCallback, useEffect, useId, useImperativeHandle, useRef, useState} from 'react';
import {ArrowUp, BookOpen, Download, History, MessageSquareText, Plus, X} from 'lucide-react';
import {open} from '@tauri-apps/plugin-dialog';
import {api} from './api';
import Markdown from 'react-markdown';
import {AnnotatedReview} from './AnnotatedReview';
import type {EssayReviewState, WriterMode} from './reviewWorkflow';
import type {AiPreferences, SessionView, SelectedVersion, VersionRef, JobState, AnalysisRecord, IssueRecord, ConversationJob, PendingChatTurn, ChatTurnReceipt} from './generated/dto';
export {ModelSettings} from './ModelSettings';
const err = (e: unknown) => e instanceof Error ? e.message : String(e);
const versionLabel = (v: VersionRef) => v.kind === 'original'
  ? `初稿第 ${v.path === 'versions/original.json' ? 1 : Number(v.path.match(/original-(\d+)/)?.[1] || 2)} 版`
  : `更改版 ${Number(v.path.match(/\d+/)?.[0] || 1)}`;
const orderedRecords = (records: AnalysisRecord[]) => [...records].sort((a, b) => b.createdAt.localeCompare(a.createdAt) || b.id.localeCompare(a.id));

function JobBox({id, onDone, onState, notice, review = false}: {id: string; onDone: (job: JobState) => void; onState?: (job: JobState) => void; notice: (s: string) => void; review?: boolean}) {
  const [job, setJob] = useState<JobState | null>(null);
  const [retryTick, setRetryTick] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const callback = useRef({onDone, onState});
  callback.current = {onDone, onState};
  useEffect(() => {
    let active = true;
    let timer: number | undefined;
    const poll = async () => {
      try {
        const j = await api.call('ai_job', {id});
        if (!active) return;
        setJob(j); setError(''); callback.current.onState?.(j);
        if (j.status === 'running' || j.status === 'queued') timer = window.setTimeout(() => void poll(), 650);
        else callback.current.onDone(j);
      } catch (e) {if (active) setError(err(e));}
    };
    void poll();
    return () => {active = false; window.clearTimeout(timer);};
  }, [id, retryTick]);
  const action = async (cmd: 'ai_retry' | 'ai_cancel') => {
    setBusy(true);
    const previous = job;
    if (cmd === 'ai_retry' && previous) {
      const queued: JobState = {...previous, status: 'queued', error: null};
      setJob(queued); callback.current.onState?.(queued);
    }
    try {
      await api.call(cmd, {id});
      setRetryTick(n => n + 1);
    } catch (e) {
      if (cmd === 'ai_retry' && previous) {setJob(previous); callback.current.onState?.(previous);}
      notice(err(e));
    } finally {setBusy(false);}
  };
  return <div className="job-box" role="status">
    <strong>{job ? (review ? {queued: '等待批阅', running: 'AI 正在批阅…', completed: '批阅已保存', failed: '批阅失败', cancelled: '已停止批阅'} : {queued: '等待回复', running: 'AI 正在思考…', completed: '回复已保存', failed: '回复失败', cancelled: '已停止回复'})[job.status] : '正在连接…'}</strong>
    {job?.error && <small>{job.error}</small>}
    {error && <><small>{error}</small><button className="secondary small" onClick={() => setRetryTick(n => n + 1)}>重新连接</button></>}
    <div className="button-line">
      {(job?.status === 'running' || job?.status === 'queued') && <button className="secondary small" disabled={busy} onClick={() => void action('ai_cancel')}>停止</button>}
      {(job?.status === 'failed' || job?.status === 'cancelled') && <button className="secondary small" disabled={busy} onClick={() => void action('ai_retry')}>{review ? '重试批阅' : '重试回复'}</button>}
    </div>
  </div>;
}

async function exportAnalysis(record: AnalysisRecord, format: 'md' | 'docx', notice: (s: string) => void) {
  try {
    const destination = await open({directory: true});
    if (typeof destination !== 'string') return;
    const path = await api.call('ai_export', {id: record.id, format, destination});
    notice(`已导出：${path}`);
  } catch (e) {notice(err(e));}
}

export function AnalysisWorkspace({many, session, notice, onConfigure}: {many: boolean; session?: SessionView; notice: (s: string) => void; onConfigure?: () => void}) {
  return many ? <MultiAnalysis notice={notice} onConfigure={onConfigure}/> : session ? <EssayWorkspace key={session.session.id} session={session} mode="revise" notice={notice} onConfigure={onConfigure}/> : null;
}

export type EssayWorkspaceHandle = {startReview: (source: SelectedVersion) => Promise<void>};
type EssayWorkspaceProps = {session: SessionView; mode: WriterMode; notice: (s: string) => void; onConfigure?: () => void; onState?: (state: EssayReviewState) => void; onModeChange?: (mode: WriterMode) => void};
export const EssayWorkspace = forwardRef<EssayWorkspaceHandle, EssayWorkspaceProps>(function EssayWorkspace({session, mode, notice, onConfigure, onState, onModeChange}, ref) {
  const sessionId = session.session.id;
  const [records, setRecords] = useState<AnalysisRecord[]>([]);
  const [activeId, setActiveId] = useState('latest');
  const [preferences, setPreferences] = useState<AiPreferences | null>(null);
  const [pending, setPending] = useState<ConversationJob | null>(null);
  const [issues, setIssues] = useState<IssueRecord[]>([]);
  const [message, setMessage] = useState('');
  const [sendingMessage, setSendingMessage] = useState('');
  const [unresolvedTurn, setUnresolvedTurn] = useState<PendingChatTurn | null>(null);
  const [checkingTurn, setCheckingTurn] = useState(false);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [showIssues, setShowIssues] = useState(false);
  const [reviewId, setReviewId] = useState('');
  const [showHistory, setShowHistory] = useState(false);
  const [showExports, setShowExports] = useState(false);
  const initialized = useRef(false);
  const mounted = useRef(false);
  const loadRequest = useRef(0);
  const scroll = useRef<HTMLDivElement>(null);
  const textarea = useRef<HTMLTextAreaElement>(null);
  const requestBusy = useRef(false);
  const reviewJob = useRef(true);
  const callbacks = useRef({onState, onModeChange});
  callbacks.current = {onState, onModeChange};
  const pendingRef = useRef(pending);
  pendingRef.current = pending;
  const load = useCallback(async () => {
    const request = ++loadRequest.current;
    if (!api.native) {setLoading(false); return;}
    try {
      const [aa, prefs, ii, jobs] = await Promise.all([
        api.call('ai_analyses'), api.call('ai_preferences'), api.call('ai_issues'), api.call('ai_conversation_jobs', {sessionId}),
      ]);
      if (!mounted.current || request !== loadRequest.current) return;
      const saved = orderedRecords(aa.filter(a => !a.many && a.sources.some(s => s.sessionId === sessionId)));
      setRecords(saved);
      setPreferences(prefs); setIssues(ii.filter(i => i.sessionId === sessionId));
      if (!initialized.current) {
        const latest = jobs.find(j => j.state.status === 'running' || j.state.status === 'queued') || (jobs[0]?.state.status !== 'completed' ? jobs[0] : undefined);
        setPending(latest || null);
        const reviewed = saved.find(r => r.review);
        if (reviewed) setActiveId(reviewed.id);
        else if (latest) setActiveId('');
        if (latest || saved.some(r => r.review)) callbacks.current.onModeChange?.('review');
        initialized.current = true;
      }
      setError('');
    } catch (e) {if (mounted.current && request === loadRequest.current) {setError(err(e)); callbacks.current.onModeChange?.('review');}}
    finally {if (mounted.current && request === loadRequest.current) setLoading(false);}
  }, [sessionId]);
  useEffect(() => {mounted.current = true; void load(); return () => {mounted.current = false; loadRequest.current++; pendingRef.current = null;};}, [load]);
  const active = activeId === 'latest' ? records[0] : records.find(r => r.id === activeId);
  const reviewRecord = records.find(r => r.id === reviewId && r.review) || records.find(r => r.review);
  const fixedVersion = [...session.versions].reverse().find(v => v.fixed);
  const sourceVersion = session.versions.find(v => v.id === active?.sources[0]?.versionId);
  const ready = !!preferences?.presets.some(p => p.slot === preferences.activeSlot);
  const waiting = busy || checkingTurn || pending?.state.status === 'running' || pending?.state.status === 'queued';
  const conversationId = active?.id;
  const reviewing = mode === 'review';
  const reviewVersion = session.versions.find(v => v.id === reviewRecord?.sources[0]?.versionId);
  useEffect(() => {callbacks.current.onState?.({loading, ready, hasReview: !!reviewRecord, hasPending: !!pending, busy: waiting});}, [loading, ready, !!reviewRecord, !!pending, waiting]);
  useImperativeHandle(ref, () => ({startReview: async source => {
    if (requestBusy.current || waiting || loading) throw Error('请等待当前请求完成。');
    if (!ready) throw Error('请先保存并启用一组模型配置。');
    if (source.sessionId !== sessionId) throw Error('送审作文已切换，请重试。');
    requestBusy.current = true; reviewJob.current = true; setBusy(true); setError('');
    try {
      const job = await api.call('ai_start_selected', {items: [source], many: false});
      setPending({state: {schemaVersion: 1, id: job.id, status: 'queued', completedItems: 0, totalItems: 1, resultId: null, error: null}, source, openingMessage: '请批阅本次固定稿。'});
    } catch (e) {setError(err(e)); throw e;}
    finally {requestBusy.current = false; setBusy(false);}
  }}), [sessionId, waiting, loading, ready]);
  useEffect(() => {
    let alive = true;
    setUnresolvedTurn(null);
    if (!conversationId || !api.native) {setCheckingTurn(false); return;}
    setCheckingTurn(true);
    void api.call('ai_pending_turn', {id: conversationId}).then(p => {if (alive) setUnresolvedTurn(p);})
      .catch(e => {if (alive) setError(err(e));}).finally(() => {if (alive) setCheckingTurn(false);});
    return () => {alive = false;};
  }, [conversationId]);
  const applyReceipt = async (id: string, receipt: ChatTurnReceipt) => {
    setRecords(rs => rs.map(r => r.id === id ? {...r, revision: receipt.revision, lastTurnId: receipt.clientTurnId,
      messages: [...r.messages.filter(m => m.id !== receipt.user.id && m.id !== receipt.reply.id), receipt.user, receipt.reply]} : r));
    setUnresolvedTurn(null); setMessage(''); setSendingMessage(''); setError('');
    if (receipt.notice) notice(receipt.notice);
    try {
      const record = await api.call('ai_analysis', {id});
      setRecords(rs => rs.map(r => r.id === id ? record : r));
    } catch {notice('回复已保存，历史详情暂未刷新，可重新加载。');}
  };
  useEffect(() => {
    const node = scroll.current;
    if (node) node.scrollTop = node.scrollHeight;
  }, [active?.id, active?.messages.length, pending?.state.id, sendingMessage, reviewing]);
  const send = async () => {
    if (requestBusy.current || unresolvedTurn || !message.trim() || waiting || !ready || (!active && !fixedVersion)) return;
    requestBusy.current = true;
    const content = message.trim();
    setBusy(true); setSendingMessage(content); setError('');
    try {
      if (active) {
        const receipt = await api.call('ai_chat', {id: active.id, message: content, clientTurnId: crypto.randomUUID()});
        await applyReceipt(active.id, receipt);
      } else {
        reviewJob.current = false;
        const job = await api.call('ai_start_conversation', {sessionId, message: content});
        setPending({state: {schemaVersion: 1, id: job.id, status: 'running', completedItems: 0, totalItems: 1, resultId: null, error: null}, source: {sessionId, versionId: fixedVersion!.id}, openingMessage: content});
      }
      setMessage('');
    } catch (e) {
      setError(err(e));
      if (active) {try {setUnresolvedTurn(await api.call('ai_pending_turn', {id: active.id}));} catch { /* Keep the original failure visible. */ }}
    } finally {requestBusy.current = false; setBusy(false); setSendingMessage(''); textarea.current?.focus();}
  };
  const recoverTurn = async () => {
    if (!active || !unresolvedTurn || requestBusy.current || waiting) return;
    requestBusy.current = true; setBusy(true); setSendingMessage(unresolvedTurn.message); setError('');
    try {await applyReceipt(active.id, await api.call('ai_retry_turn', {id: active.id, clientTurnId: unresolvedTurn.clientTurnId}));}
    catch (e) {setError(err(e)); try {setUnresolvedTurn(await api.call('ai_pending_turn', {id: active.id}));} catch { /* Keep pending recovery available. */ }}
    finally {requestBusy.current = false; setBusy(false); setSendingMessage('');}
  };
  const jobDone = async (job: JobState) => {
    if (pendingRef.current?.state.id !== job.id) return;
    if (job.status === 'completed' && job.resultId) {
      try {
        const record = await api.call('ai_analysis', {id: job.resultId});
        if (pendingRef.current?.state.id !== job.id) return;
        await load();
        if (pendingRef.current?.state.id !== job.id) return;
        setRecords(rs => orderedRecords([record, ...rs.filter(r => r.id !== record.id)]));
        setActiveId(record.id); setReviewId(record.id); setPending(null);
        if (reviewJob.current) callbacks.current.onModeChange?.('review');
      } catch (e) {if (pendingRef.current?.state.id === job.id) setError(err(e));}
    } else setPending(p => p?.state.id === job.id ? {...p, state: job} : p);
  };
  const newConversation = () => {setPending(null); setActiveId(''); setUnresolvedTurn(null); setShowHistory(false); setError(''); setMessage(''); textarea.current?.focus();};
  return <div className="essay-chat ai-page" aria-label="作文 AI 对话">
    <div className="chat-toolbar">
      <span>{reviewing ? '批阅结果' : active ? '围绕批阅结果继续讨论' : '新对话'}</span>
      <div>
        <button className="icon-button" title={reviewing ? '批阅记录' : '聊天记录'} aria-label={reviewing ? '批阅记录' : '聊天记录'} aria-expanded={showHistory} onClick={() => setShowHistory(s => !s)}><History size={16}/></button>
        {!reviewing && <button className="icon-button" title="新对话" aria-label="新对话" disabled={waiting || loading} onClick={newConversation}><Plus size={17}/></button>}
        <button className="icon-button" title="导出记录" aria-label="导出记录" aria-expanded={showExports} disabled={reviewing ? !reviewRecord : !active} onClick={() => setShowExports(s => !s)}><Download size={16}/></button>
      </div>
    </div>
    {showHistory && <div className="chat-history-menu" aria-label="已保存的对话">
      <strong>{reviewing ? '批阅记录' : '聊天记录'}</strong>
      {records.length ? records.filter(r => !reviewing || r.review).map((r, n) => <button key={r.id} aria-pressed={(reviewing ? reviewRecord?.id : active?.id) === r.id} disabled={waiting} onClick={() => {if (reviewing) setReviewId(r.id); setActiveId(r.id); setShowHistory(false); setError('');}}><span>{r.openingMessage || `作文分析 ${records.length - n}`}</span><small>{r.createdAt.replace('T', ' ').slice(0, 16)}</small></button>) : <p>还没有已保存的记录。</p>}
    </div>}
    {showExports && (reviewing ? reviewRecord : active) && <div className="chat-export-menu button-line"><button className="secondary small" onClick={() => void exportAnalysis((reviewing ? reviewRecord : active)!, 'md', notice)}>导出 Markdown</button><button className="secondary small" onClick={() => void exportAnalysis((reviewing ? reviewRecord : active)!, 'docx', notice)}>导出 Word</button></div>}
    {error && <div className="ai-inline-error" role="alert">{error}{api.native && <button className="text-button" onClick={() => {if (pending?.state.status === 'completed') void jobDone(pending.state); else void load();}}>重新加载</button>}</div>}
    <div className="review-page" hidden={!reviewing} aria-label="批阅页面">
      {pending && <div className="review-job"><JobBox review key={pending.state.id} id={pending.state.id} onState={j => setPending(p => p?.state.id === j.id ? {...p, state: j} : p)} onDone={j => void jobDone(j)} notice={notice}/></div>}
      <div className="review-snapshot" hidden={!!pending}>
        {reviewRecord ? <AnnotatedReview key={reviewRecord.id} record={reviewRecord} versionName={reviewVersion ? versionLabel(reviewVersion) : '本次固定版本'} hasQuestion={!!session.session.questionId} notice={notice}/> : <div className="review-empty" role="status">{loading ? '正在加载批阅记录…' : busy ? '正在提交批阅…' : '点击“批阅”提交作文，完成后将在这里显示原文与批注。'}</div>}
      </div>
      {pending && !waiting && reviewRecord && <button className="text-button review-previous" onClick={() => setPending(null)}>查看上次批阅结果</button>}
    </div>
    <div hidden={reviewing} ref={scroll} className="conversation-scroll" role="log" aria-label="对话内容" aria-busy={waiting}>
      {loading ? <p className="chat-status" role="status">正在加载聊天记录…</p> : <>
        {active && <>
          <div className="conversation-divider">{sourceVersion ? versionLabel(sourceVersion) : '已固定的作文版本'} · {active.createdAt.slice(0, 10)}</div>
          {active.review ? <div className="chat-review-context"><strong>已附上本次批阅结果</strong><p>{active.review.summary || '可围绕原文和批注提问，完整批注可在“批阅”页面查看。'}</p></div> : <>{active.openingMessage && <ChatBubble role="user" content={active.openingMessage}/>}<ChatBubble role="assistant" content={active.markdown}/></>}
          {active.messages.map(m => <ChatBubble key={m.id} role={m.role} content={m.content}/>)}
        </>}
        {!active && !pending && !sendingMessage && <div className="conversation-empty"><span className="conversation-empty-icon"><MessageSquareText size={24}/></span><h2>聊聊这篇作文</h2><p>提出你的问题，或让 AI 帮你分析。<br/>作文和题目会随消息一起附上。</p>{fixedVersion && <button className="chat-suggestion" disabled={!ready || !api.native} onClick={() => {setMessage('请分析这篇作文，给出具体的修改建议。'); textarea.current?.focus();}}>帮我分析这篇作文</button>}</div>}
        {pending && <p className="chat-status" role="status">{waiting ? '批阅正在进行，可切换到“批阅”查看进度。' : '本次请求未完成，可切换到“批阅”重试。'}</p>}
        {sendingMessage && <><ChatBubble role="user" content={sendingMessage}/><p className="chat-status" role="status">{unresolvedTurn?.status === 'received' ? '正在保存已收到的回复…' : 'AI 正在思考…'}</p></>}
      </>}
    </div>
    <div hidden={reviewing} className="conversation-footer">
      {!api.native ? <p className="chat-status">请在桌面应用中发送消息。</p> : !loading && !ready ? <div className="chat-setup-hint"><span>先保存并启用一组配置，即可开始对话。</span>{onConfigure && <button className="text-button" onClick={onConfigure}>前往语言大模型配置</button>}</div> : !fixedVersion && !active ? <p className="chat-status">请先固定作文版本，再开始对话。</p> : <p className="chat-context">{active ? `继续围绕${sourceVersion ? versionLabel(sourceVersion) : '本次作文版本'}提问` : `将附上${fixedVersion ? versionLabel(fixedVersion) : '作文'}`}</p>}
      {unresolvedTurn && <div className="pending-turn-notice" role="status"><strong>{unresolvedTurn.status === 'received' ? '回复已收到，等待保存' : '上次模型调用结果未确认'}</strong><p>{unresolvedTurn.status === 'received' ? '恢复会继续保存已有回复，不会再次请求模型。' : '重新发送会从已保存历史恢复上下文，原请求可能已计费。'}</p><button type="button" className="secondary small" disabled={waiting} onClick={() => void recoverTurn()}>{unresolvedTurn.status === 'received' ? '恢复已收到的回复' : '重新发送原问题'}</button></div>}
      {active && <p className="chat-context-mode" title="下一轮按当前启用的配置管理上下文。服务端正常接续仅发送新问题；问答均保存在本地。">{active.contextState ? '上轮使用服务端接续' : '上轮使用本地历史'} · 问答已保存</p>}
      <form className="chat-composer" onSubmit={e => {e.preventDefault(); void send();}}>
        <textarea ref={textarea} rows={3} value={message} disabled={waiting || loading || !!unresolvedTurn} aria-label="聊天消息" placeholder={unresolvedTurn ? '请先处理上次聊天请求…' : '输入你想讨论的问题…'} onChange={e => setMessage(e.target.value)} onKeyDown={e => {if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing && e.nativeEvent.keyCode !== 229) {e.preventDefault(); void send();}}}/>
        <div className="chat-composer-bottom"><span>Enter 发送 · Shift + Enter 换行</span><button type="submit" className="primary chat-send" aria-label="发送消息" disabled={!api.native || loading || !ready || waiting || !!unresolvedTurn || !message.trim() || (!active && !fixedVersion)}><ArrowUp size={18}/></button></div>
      </form>
      <div className="chat-bottom-bar"><button className="text-button issues-trigger" onClick={() => setShowIssues(true)}><BookOpen size={15}/>单篇问题记录{issues.length > 0 && <span>{issues.length}</span>}</button><span className="chat-saved-note">对话自动保存</span></div>
    </div>
    {showIssues && <IssueDialog issues={issues} title={session.session.title} onClose={() => setShowIssues(false)} onSaved={() => void load()} notice={notice}/>}
  </div>;
});

function ChatBubble({role, content}: {role: 'user' | 'assistant'; content: string}) {
  return <article className={`conversation-message ${role}`}><span className="message-author">{role === 'user' ? '我' : 'AI'}</span><div className="message-content">{role === 'assistant' ? <Markdown skipHtml components={{a: ({children, href, title}) => <a href={href} title={title} target="_blank" rel="noreferrer">{children}</a>}}>{content}</Markdown> : content}</div></article>;
}

function IssueDialog({issues, title, onClose, onSaved, notice}: {issues: IssueRecord[]; title: string; onClose: () => void; onSaved: () => void; notice: (s: string) => void}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const heading = useId();
  useEffect(() => {
    const previous = document.activeElement;
    const element = dialog.current!;
    element.showModal();
    return () => {element.close(); if (previous instanceof HTMLElement && previous.isConnected) previous.focus();};
  }, []);
  return <dialog ref={dialog} className="issues-dialog" aria-labelledby={heading} onCancel={e => {e.preventDefault(); onClose();}}>
    <div className="issues-dialog-head"><div><h2 id={heading}>单篇问题记录</h2><p>{title}</p></div><button className="icon-button" aria-label="关闭问题记录" onClick={onClose} autoFocus><X size={19}/></button></div>
    <div className="issues-dialog-body">{issues.length ? issues.map(i => <IssueEditor key={i.id} issue={i} onSaved={onSaved} notice={notice}/>) : <div className="issues-empty"><BookOpen size={28}/><h3>还没有问题记录</h3><p>对话生成的单篇问题归纳会保存在这里。<br/>可在语言大模型配置中开启自动记录。</p></div>}</div>
  </dialog>;
}

function MultiAnalysis({notice, onConfigure}: {notice: (s: string) => void; onConfigure?: () => void}) {
  const [views, setViews] = useState<SessionView[]>([]);
  const [preferences, setPreferences] = useState<AiPreferences | null>(null);
  const [selected, setSelected] = useState<Record<string, boolean>>({});
  const [records, setRecords] = useState<AnalysisRecord[]>([]);
  const [activeId, setActiveId] = useState('');
  const [jobId, setJobId] = useState('');
  const [busy, setBusy] = useState(false);
  const initialized = useRef(false);
  const load = useCallback(async () => {
    if (!api.native) return;
    const [prefs, ss, aa, jobs] = await Promise.all([api.call('ai_preferences'), api.call('list_sessions'), api.call('ai_analyses'), api.call('ai_many_jobs')]);
    setPreferences(prefs); setRecords(orderedRecords(aa.filter(a => a.many)));
    if (!initialized.current) {
      const job = jobs.find(j => j.status === 'running' || j.status === 'queued') || (jobs[0]?.status !== 'completed' ? jobs[0] : undefined);
      if (job) {setJobId(job.id); setBusy(job.status === 'running' || job.status === 'queued');}
      initialized.current = true;
    }
    setViews(await Promise.all(ss.map(s => api.call('open_session', {sessionId: s.id}))));
  }, []);
  useEffect(() => {void load().catch(e => notice(err(e)));}, [load]);
  const list: SelectedVersion[] = Object.entries(selected).filter(([, checked]) => checked).map(([key]) => {const [sessionId, versionId] = key.split('|'); return {sessionId, versionId};});
  const active = records.find(r => r.id === activeId);
  const start = async () => {
    if (busy) return;
    setBusy(true);
    try {const job = await api.call('ai_start_selected', {items: list, many: true}); setJobId(job.id);} catch (e) {setBusy(false); notice(err(e));}
  };
  return <div className="page-wrap narrow ai-page"><div className="page-heading"><div><span className="eyebrow">跨篇复盘</span><h1>多篇分析</h1><p>勾选固定版本，使用语言大模型配置中当前启用的配置生成报告。</p></div></div>
    {!api.native && <div className="empty">真实模型调用请在桌面应用中操作。</div>}
    <section className="ai-section"><h2>选择来源版本</h2><div className="ai-sources">{views.map(v => <div key={v.session.id}><strong>{v.session.title}</strong>{v.versions.filter(ver => ver.fixed).map(ver => {const key = `${v.session.id}|${ver.id}`; return <label key={ver.id}><input type="checkbox" checked={!!selected[key]} onChange={e => setSelected(s => ({...s, [key]: e.target.checked}))}/>{versionLabel(ver)} · {ver.wordCount} 词</label>;})}{!v.versions.some(ver => ver.fixed) && <small>尚无已固定版本</small>}</div>)}</div>
      {!preferences?.activeSlot && onConfigure && <button className="text-button" onClick={onConfigure}>前往语言大模型配置</button>}
      <div className="button-line multi-start"><button className="primary" disabled={!api.native || !list.length || !preferences?.activeSlot || busy} onClick={() => void start()}>{busy ? '提交中…' : '开始分析'}</button><span className="subtle">已选 {list.length} 个固定版本</span></div>
      {jobId && <JobBox key={jobId} id={jobId} onState={job => setBusy(job.status === 'running' || job.status === 'queued')} onDone={job => {setBusy(false); void load().catch(e => notice(err(e))); if (job.resultId) setActiveId(job.resultId);}} notice={notice}/>}
    </section>
    <section className="ai-section"><h2>总体分析报告</h2>{records.length ? <><select className="analysis-picker" aria-label="选择总体分析报告" value={activeId} onChange={e => setActiveId(e.target.value)}><option value="">选择已归档分析</option>{records.map(r => <option key={r.id} value={r.id}>{r.title} · {r.createdAt.slice(0, 10)}</option>)}</select>{active && <><div className="analysis-meta">来源：{active.sources.map(s => `${s.sessionId.slice(-8)}/${s.versionId.slice(-8)}`).join('、')}</div><article className="analysis-text">{active.markdown}</article><div className="button-line"><button className="secondary small" onClick={() => void exportAnalysis(active, 'md', notice)}>导出 Markdown</button><button className="secondary small" onClick={() => void exportAnalysis(active, 'docx', notice)}>导出 Word</button></div></>}</> : <p>暂无已归档报告。</p>}</section>
  </div>;
}

function IssueEditor({issue, onSaved, notice}: {issue: IssueRecord; onSaved: () => void; notice: (s: string) => void}) {
  const [value, setValue] = useState(issue.text);
  const [busy, setBusy] = useState(false);
  useEffect(() => setValue(issue.text), [issue.id, issue.text]);
  const save = async (remove: boolean) => {
    setBusy(true);
    try {await api.call('ai_edit_issue', {id: issue.id, text: value, remove}); onSaved(); notice(remove ? '问题记录已移除' : '问题记录已更新');} catch (e) {notice(err(e));} finally {setBusy(false);}
  };
  return <div className="issue"><small>作文 {issue.sessionId.slice(-8)} · 版本 {issue.versionId.slice(-8)} · 分析 {issue.analysisId.slice(-8)}</small><textarea rows={5} aria-label="问题记录内容" value={value} disabled={busy} onChange={e => setValue(e.target.value)}/><div className="button-line"><button className="secondary small" disabled={busy || value === issue.text} onClick={() => void save(false)}>保存编辑</button><button className="secondary small" disabled={busy} onClick={() => void save(true)}>移除</button></div></div>;
}
export function IssueRegister({notice}: {notice: (s: string) => void}) {
  const [items, setItems] = useState<IssueRecord[]>([]);
  const load = () => api.call('ai_issues').then(setItems).catch(e => notice(err(e)));
  useEffect(() => {if (api.native) void load();}, []);
  return <div className="page-wrap narrow ai-page"><div className="page-heading"><div><span className="eyebrow">复习归档</span><h1>单篇问题记录</h1><p>每条记录带作文、版本与分析来源。可编辑或移除；原分析仍保留。</p></div></div><section className="ai-section">{items.length ? items.map(i => <IssueEditor key={i.id} issue={i} onSaved={() => void load()} notice={notice}/>) : <p>尚无问题记录。可在语言大模型配置中开启自动记录。</p>}</section></div>;
}
