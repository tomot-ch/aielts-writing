import {useCallback,useEffect,useRef,useState} from 'react';
import {BookOpen, ChevronDown, ChevronRight, Clock3, Download, FilePenLine, FolderOpen, LayoutPanelLeft, Maximize2, MessageSquareText, Minimize2, Moon, Plus, Settings2, Sun, Trash2, Upload, X} from 'lucide-react';
import {open} from '@tauri-apps/plugin-dialog';import {getCurrentWindow} from '@tauri-apps/api/window';import {listen} from '@tauri-apps/api/event';import {openPath} from '@tauri-apps/plugin-opener';
import {api,type InkColor,type Question,type SessionSummary,type SessionView,type Startup,type TextRun} from './api';
import {insertColored,readRuns,renderRuns,wordCount} from './editor';
import {ModelSettings,AnalysisWorkspace,EssayWorkspace,IssueRegister,type EssayWorkspaceHandle} from './V2';
import {SessionSave} from './sessionSave';
import {emptyReviewState,prepareReviewSource,type EssayReviewState,type WriterMode} from './reviewWorkflow';
import './review-workflow.css';
import {DeleteDialog} from './DeleteDialog';
import {QuestionBank} from './QuestionBank';
import {Lobby} from './Lobby';
type Page='home'|'history'|'questions'|'import'|'editor'|'export'|'ai'|'many'|'issues'|'settings';
type Theme='system'|'light'|'dark';
type DeleteTarget={kind:'session';item:SessionSummary}|{kind:'question';item:Question};
const label=(t:string|null)=>t==='task1'?'Task 1':t==='task2'?'Task 2':'自由写作';
const errorText=(e:unknown)=>e instanceof Error?e.message:String(e);
// WebKit inline predictions need their own switch, separate from spellcheck/autocorrect.
// Use the HTML spelling until React's DOM types include this attribute.
const writingInputAttributes={writingsuggestions:'false'} as const;
export default function App(){
 const [startup,setStartup]=useState<Startup|null>(null),[rootChoice,setRootChoice]=useState(''),[page,setPage]=useState<Page>('home'),[prep,setPrep]=useState(false);
 const [sessions,setSessions]=useState<SessionSummary[]>([]),[questions,setQuestions]=useState<Question[]>([]),[selectedQuestion,setSelectedQuestion]=useState<Question|null>(null),[questionImg,setQuestionImg]=useState('');
 const [questionError,setQuestionError]=useState('');
 const [view,setView]=useState<SessionView|null>(null),[runs,setRuns]=useState<TextRun[]>([]),[ink,setInk]=useState<InkColor>('black'),[notice,setNotice]=useState(''),[saveState,setSaveState]=useState('已保存');
 const [theme,setTheme]=useState<Theme>((localStorage.getItem('ielts-theme') as Theme)||'system');
 const [hideHomeText,setHideHomeText]=useState(localStorage.getItem('ielts-hide-home-text')==='true');
 const [examMode,setExamMode]=useState(false);
 const [writerMode,setWriterMode]=useState<WriterMode>('write'),[reviewState,setReviewState]=useState<EssayReviewState>(emptyReviewState);
 const analysis=useRef<EssayWorkspaceHandle>(null);
 const [showQuestion,setShowQuestion]=useState(true),[showEssay,setShowEssay]=useState(true),[showChat,setShowChat]=useState(false);
 const [leftWidth,setLeftWidth]=useState(Number(localStorage.getItem('ielts-left-width')||38)),[chatWidth,setChatWidth]=useState(Number(localStorage.getItem('ielts-chat-width')||38));
 const [importItems,setImportItems]=useState<{path:string;taskType:'task1'|'task2';title:string}[]>([]);
 const [tick,setTick]=useState(0),[loadSeq,setLoadSeq]=useState(0),[exportSelection,setExportSelection]=useState<Record<string,boolean>>({});
 const editor=useRef<HTMLDivElement>(null),runsRef=useRef<TextRun[]>([]),debounce=useRef<number|undefined>(undefined),viewRef=useRef<SessionView|null>(null);
 const saverRef=useRef<SessionSave|null>(null);
 if(!saverRef.current)saverRef.current=new SessionSave((sessionId,expectedRevision,runs)=>api.call('save_session',{sessionId,expectedRevision,runs}),setSaveState,e=>setNotice(`保存失败：${errorText(e)}`));
 const saver=saverRef.current;
 const transitionRef=useRef(false),submissionPendingRef=useRef(false);
 const [transitioning,setTransitioning]=useState(false),[submissionPending,setSubmissionPending]=useState(false);
 const [deleteTarget,setDeleteTarget]=useState<DeleteTarget|null>(null),[deleting,setDeleting]=useState(false);
 const deleteBusy=useRef(false),previewSeq=useRef(0);
 const pendingQuickRef=useRef(false),pendingTitleRef=useRef(''),pendingCreatedIdRef=useRef<string|null>(null),creatingQuickRef=useRef<Promise<void>|null>(null);
 useEffect(()=>{document.documentElement.dataset.theme=theme;localStorage.setItem('ielts-theme',theme)},[theme]);
 useEffect(()=>{localStorage.setItem('ielts-hide-home-text',String(hideHomeText))},[hideHomeText]);
 useEffect(()=>{localStorage.setItem('ielts-left-width',String(leftWidth))},[leftWidth]);useEffect(()=>{localStorage.setItem('ielts-chat-width',String(chatWidth))},[chatWidth]);
 useEffect(()=>{api.call('startup').then(async s=>{setStartup(s);setRootChoice(s.root||s.recommendedRoot);if(s.configured){await api.call('seed_questions').catch(e=>setNotice(`内置题目检查失败：${errorText(e)}`));await refresh()}}).catch(e=>setNotice(errorText(e)))},[]);
 useEffect(()=>{const id=window.setInterval(()=>setTick(t=>t+1),1000);return()=>clearInterval(id)},[]);
 useEffect(()=>{if(editor.current&&view){renderRuns(editor.current,runsRef.current)}},[loadSeq,showEssay]);
 useEffect(()=>{const onLeave=()=>{if(saver.hasUnsaved)void flushSave()};window.addEventListener('blur',onLeave);return()=>window.removeEventListener('blur',onLeave)},[]);
 useEffect(()=>{
  if(!api.native)return;
  const saveThenQuit=()=>runSavedAction(async()=>{await api.call('quit_app')});
  const close=getCurrentWindow().onCloseRequested(event=>{event.preventDefault();void saveThenQuit()});
  const quit=listen('before-quit',()=>void saveThenQuit());
  return()=>{void close.then(unlisten=>unlisten());void quit.then(unlisten=>unlisten())};
 },[]);
 const refresh=async()=>{const [ss,qs]=await Promise.all([api.call('list_sessions'),api.call('list_questions')]);setSessions(ss);setQuestions(qs)};
 const configure=(path:string,migrate=false)=>runSavedAction(async()=>{await api.call('set_data_root',{path,migrate});const s=await api.call('startup');setStartup(s);setRootChoice(s.root||s.recommendedRoot);await api.call('seed_questions');await refresh();setNotice('数据目录已准备好，首批题目已检查。')});
 const ensureQuickCreated=async()=>{
  if(!pendingQuickRef.current)return;
  if(creatingQuickRef.current)return creatingQuickRef.current;
  const task=(async()=>{
   if(!pendingCreatedIdRef.current){const args=pendingTitleRef.current?{title:pendingTitleRef.current}:{};const s=await api.call('create_quick',args);pendingCreatedIdRef.current=s.id}
   const v=await api.call('open_session',{sessionId:pendingCreatedIdRef.current});
   saver.attach(v.session.id,v.saveRevision);viewRef.current=v;pendingQuickRef.current=false;pendingCreatedIdRef.current=null;setView(v);await refresh();
  })();
  creatingQuickRef.current=task;
  try{await task}finally{creatingQuickRef.current=null}
 };
 const flushSave=async()=>{
  window.clearTimeout(debounce.current);
  if(pendingQuickRef.current&&saver.hasUnsaved){if(!runsRef.current.some(x=>x.text.length>0)&&!creatingQuickRef.current){saver.discardEmptyQuick();setSaveState('输入后保存');return true}try{await ensureQuickCreated()}catch(e){setSaveState('保存失败');setNotice(`创建作文失败：${errorText(e)}`);return false}}
  return saver.flush();
 };
 const runSavedAction=async(action:()=>Promise<void>)=>{
  if(transitionRef.current)return;
  transitionRef.current=true;setTransitioning(true);
  // Commit any active IME composition before freezing input and draining saves.
  editor.current?.blur();
  try{if(await flushSave())await action()}catch(e){setNotice(errorText(e))}
  finally{transitionRef.current=false;setTransitioning(false)}
 };
 const scheduleSave=()=>{window.clearTimeout(debounce.current);debounce.current=window.setTimeout(()=>void flushSave(),350)};
 const onEdit=()=>{if(!editor.current)return;const next=readRuns(editor.current);runsRef.current=next;setRuns(next);saver.update(next);if(pendingQuickRef.current&&next.some(x=>x.text.length>0))void ensureQuickCreated().then(()=>void flushSave()).catch(e=>{setSaveState('保存失败');setNotice(`创建作文失败：${errorText(e)}`)});else scheduleSave()};
 const leaveExamMode=async()=>{if(!examMode)return;try{if(api.native)await getCurrentWindow().setFullscreen(false);else if(document.fullscreenElement)await document.exitFullscreen()}catch(e){setNotice(errorText(e))}setExamMode(false)};
 const toggleExamMode=async()=>{if(examMode){await leaveExamMode();return}try{if(api.native)await getCurrentWindow().setFullscreen(true);else if(document.fullscreenEnabled)await document.documentElement.requestFullscreen();setExamMode(true)}catch(e){setNotice(`无法进入全屏：${errorText(e)}`)}};
 const loadSession=async(id:string,preserveWorkspace=false)=>{
  await leaveExamMode();
  const v=await api.call('open_session',{sessionId:id});
  const image=v.session.questionId?await api.call('session_question_image',{sessionId:id}):'';
  saver.reset(v.session.id,v.saveRevision,v.activeRuns);
  pendingQuickRef.current=false;pendingCreatedIdRef.current=null;submissionPendingRef.current=false;setSubmissionPending(false);
  viewRef.current=v;runsRef.current=v.activeRuns;setView(v);setRuns(v.activeRuns);setSaveState('已保存');if(!preserveWorkspace){setShowQuestion(!!v.session.questionId);setShowEssay(true);setShowChat(true);setWriterMode('write');setReviewState(emptyReviewState);setTick(0)}setLoadSeq(x=>x+1);setPage('editor');setQuestionImg(image);
 };
 const openSession=(id:string)=>runSavedAction(()=>loadSession(id));
 const createQuick=()=>runSavedAction(async()=>{
  await leaveExamMode();saver.reset(null,0,[]);pendingQuickRef.current=true;pendingCreatedIdRef.current=null;pendingTitleRef.current='';submissionPendingRef.current=false;setSubmissionPending(false);viewRef.current=null;runsRef.current=[];
  const now=new Date().toISOString();const v:SessionView={session:{id:'pending-quick',title:'未命名写作',questionId:null,taskType:null,status:'draft',wordCount:0,updatedAt:now,folder:''},activeRuns:[],saveRevision:0,versions:[],timerEnabled:false,remainingSeconds:null,workingVersionId:null};setView(v);setRuns([]);setSaveState('输入后保存');setShowQuestion(false);setShowEssay(true);setShowChat(true);setWriterMode('write');setReviewState(emptyReviewState);setQuestionImg('');setLoadSeq(x=>x+1);setPage('editor');
 });
 const createFrom=(q:Question)=>runSavedAction(async()=>{const s=await api.call('create_from_question',{questionId:q.id});await refresh();await loadSession(s.id)});
 const navigate=(next:Page)=>runSavedAction(async()=>{await leaveExamMode();if(pendingQuickRef.current){pendingQuickRef.current=false;setView(null)}setPage(next);if(next==='questions'||next==='export'||next==='home'||next==='history')await refresh()});
 const review=(again=false)=>runSavedAction(async()=>{
  await leaveExamMode();
  if(!again&&(reviewState.hasReview||reviewState.hasPending||reviewState.busy)){setWriterMode('review');return}
  if(!runsRef.current.map(r=>r.text).join('').trim()){setNotice('请先写完文章，再点击批阅。');return}
  if(reviewState.loading){setNotice('正在加载批阅配置，请稍候。');return}
  if(!reviewState.ready){setNotice('请先在“语言大模型配置”中保存并启用一组配置。');return}
  if(reviewState.busy)return;
  const id=viewRef.current?.session.id;if(!id||!analysis.current)return;
  submissionPendingRef.current=true;setSubmissionPending(true);
  try{
   // Re-read after saves: a previous interrupted submission may already have fixed the draft.
   const current=await api.call('open_session',{sessionId:id});
   const source=await prepareReviewSource(current,{
    submitOriginal:sessionId=>api.call('submit_original',{sessionId}),
    finishRevision:sessionId=>api.call('finish_revision',{sessionId}),
   });
   await loadSession(id,true);setWriterMode('review');
   await analysis.current.startReview(source);
  }finally{submissionPendingRef.current=false;setSubmissionPending(false)}
 });
 const revise=()=>runSavedAction(async()=>{
  if(!reviewState.hasReview)return;
  await leaveExamMode();
  const current=viewRef.current;if(!current)return;
  if(current.session.status==='original_submitted'){
   const source=[...current.versions].reverse().find(v=>v.fixed);
   if(!source)throw Error('没有可修改的固定稿件。');
   await api.call('begin_revision',{sessionId:current.session.id,sourceVersionId:source.id});
   await loadSession(current.session.id,true);
  }
  setShowQuestion(false);setShowEssay(true);setShowChat(true);setWriterMode('revise');
 });
 const finish=()=>runSavedAction(async()=>{const id=viewRef.current?.session.id;if(!id)return;await api.call('finish_revision',{sessionId:id});await loadSession(id,true);setNotice('修改版本已保存，原稿和批阅记录均已保留。点击“修改”可继续建立新版本。')});
 const changeTitle=async(title:string)=>{
  const current=viewRef.current||view;if(!current||!title.trim()||title.trim()===current.session.title)return;
  if(pendingQuickRef.current){pendingTitleRef.current=title.trim();setView({...current,session:{...current.session,title:title.trim()}});return}
  try{const renamed=await api.call('rename_session',{sessionId:current.session.id,title});if(viewRef.current?.session.id===renamed.id){const next={...viewRef.current,session:{...viewRef.current.session,title:renamed.title,updatedAt:renamed.updatedAt}};viewRef.current=next;setView(next)}await refresh()}catch(e){setNotice(errorText(e))}
 };
 const pickImport=async()=>{if(!api.native){setNotice('浏览器原型中的文件导入仅在桌面应用内可用。');return}const paths=await open({multiple:true,filters:[{name:'题目图片',extensions:['png','jpg','jpeg','webp']} ]});if(!paths)return;const list=Array.isArray(paths)?paths:[paths];setImportItems(list.map(path=>{const name=path.split(/[\\/]/).pop()?.replace(/\.[^.]+$/,'')||'导入题目';return {path,title:name,taskType:/task[ _-]?2/i.test(name)?'task2':'task1'}}));setPage('import')};
 const finishImport=async()=>{try{const result=await api.call('import_images',{items:importItems});setImportItems([]);await refresh();setNotice(`已导入 ${result.imported.length} 张；跳过 ${result.skipped.length} 张。${result.skipped[0]?` ${result.skipped[0].reason}`:''}`)}catch(e){setNotice(errorText(e))}};
 const previewQuestion=async(q:Question)=>{const sequence=++previewSeq.current;setSelectedQuestion(q);setQuestionImg('');setQuestionError('');try{const image=await api.call('question_image',{questionId:q.id});if(sequence===previewSeq.current)setQuestionImg(image)}catch(e){if(sequence===previewSeq.current){setQuestionError(errorText(e));setNotice(errorText(e))}}};
 const deleteSelected=async()=>{
  if(!deleteTarget||deleteBusy.current||transitionRef.current)return;
  const target=deleteTarget;deleteBusy.current=true;setDeleting(true);
  try{await runSavedAction(async()=>{
   if(target.kind==='session'){
    await api.call('delete_session',{sessionId:target.item.id});
    setSessions(items=>items.filter(s=>s.id!==target.item.id));
    setExportSelection(items=>Object.fromEntries(Object.entries(items).filter(([key])=>!key.startsWith(`${target.item.id}|`))));
    if(viewRef.current?.session.id===target.item.id){saver.reset(null,0,[]);viewRef.current=null;runsRef.current=[];setView(null);setRuns([]);submissionPendingRef.current=false;setSubmissionPending(false)}
   }else{
    await api.call('delete_question',{questionId:target.item.id});
    setQuestions(items=>items.filter(q=>q.id!==target.item.id));
    if(selectedQuestion?.id===target.item.id){previewSeq.current++;setSelectedQuestion(null);setQuestionImg('')}
   }
   setDeleteTarget(null);setNotice(`${target.kind==='session'?'作文':'题目'}已移入回收站。`);
  })}finally{deleteBusy.current=false;setDeleting(false)}
 };
 const exportFiles=async()=>{if(!api.native){setNotice('Word 导出请在桌面应用中操作。');return}const ids=Object.keys(exportSelection).filter(k=>exportSelection[k]);if(!ids.length){setNotice('请先勾选至少一个已固定稿件。');return}const destination=await open({directory:true});if(!destination||Array.isArray(destination))return;let n=0;for(const key of ids){const [sessionId,versionId]=key.split('|');try{await api.call('export_docx',{sessionId,versionId,destination});n++}catch(e){setNotice(`导出失败：${errorText(e)}`)}}setNotice(`已导出 ${n} 个 Word 文件到 ${destination}`)};
 const openRoot=async()=>{if(startup?.root&&api.native)await openPath(startup.root)};
 const openTrash=async()=>{if(startup?.root&&api.native)try{await openPath(`${startup.root}/回收站`)}catch(e){setNotice(errorText(e))}};
 const pickRoot=async()=>{if(!api.native)return;const p=await open({directory:true});if(typeof p==='string')setRootChoice(p)};
 const toggleTimer=async()=>{if(!view||transitionRef.current)return;try{const v=await api.call('set_timer',{sessionId:view.session.id,enabled:!view.timerEnabled});viewRef.current=v;setView(v);setTick(0)}catch(e){setNotice(errorText(e))}};
 const onBeforeInput=(e:React.FormEvent<HTMLDivElement>)=>{const event=e.nativeEvent as InputEvent;if(transitionRef.current||submissionPendingRef.current||view?.session.status==='original_submitted'){event.preventDefault();return}if(event.isComposing)return;if(event.inputType==='insertText'&&event.data){event.preventDefault();insertColored(e.currentTarget,event.data,ink);onEdit()}if(event.inputType==='insertParagraph'){event.preventDefault();insertColored(e.currentTarget,'\n',ink);onEdit()}};
 const onPaste=(e:React.ClipboardEvent<HTMLDivElement>)=>{e.preventDefault();if(transitionRef.current||submissionPendingRef.current||view?.session.status==='original_submitted')return;insertColored(e.currentTarget,e.clipboardData.getData('text/plain'),ink);onEdit()};
 const startDrag=useCallback((side:'left'|'right',event:React.PointerEvent)=>{event.preventDefault();const container=(event.currentTarget.parentElement as HTMLElement);const rect=container.getBoundingClientRect();const move=(e:PointerEvent)=>{if(side==='left')setLeftWidth(Math.min(65,Math.max(22,(e.clientX-rect.left)/rect.width*100)));else setChatWidth(Math.min(38,Math.max(19,(rect.right-e.clientX)/rect.width*100)))};const stop=()=>{window.removeEventListener('pointermove',move);window.removeEventListener('pointerup',stop)};window.addEventListener('pointermove',move);window.addEventListener('pointerup',stop)},[]);
 const remain=view?.timerEnabled?Math.max(-9999,(view.remainingSeconds??0)-tick):null;const timeText=remain===null?'计时关闭':remain<0?`已超时 ${Math.floor(-remain/60)}:${String(-remain%60).padStart(2,'0')}`:`${Math.floor(remain/60)}:${String(remain%60).padStart(2,'0')}`;
 return <div className={`app-shell ${examMode&&page==='editor'?'exam-mode':''}`}>
  <aside className="sidebar">
   <div className="nav-group-label">写作</div><button className="nav-item" onClick={createQuick}><Plus size={17}/>直接开始写作</button><button className={`nav-item ${page==='questions'?'active':''}`} onClick={()=>void navigate('questions')}><BookOpen size={17}/>从题库选择题目进行写作</button><button className={`nav-item ${page==='history'?'active':''}`} onClick={()=>void navigate('history')}><FilePenLine size={17}/>我的作文</button>
   <button className="nav-group-label group-button" onClick={()=>setPrep(!prep)}>准备&其他 {prep?<ChevronDown size={14}/>:<ChevronRight size={14}/>}</button>
   {prep&&<><button className={`nav-item ${page==='import'?'active':''}`} onClick={()=>void navigate('import')}><Upload size={17}/>下载或自行上传题目</button><button className={`nav-item ${page==='ai'?'active':''}`} onClick={()=>void navigate('ai')}><Settings2 size={17}/>语言大模型配置</button><button className={`nav-item ${page==='export'?'active':''}`} onClick={()=>void navigate('export')}><Download size={17}/>导出</button><button className={`nav-item ${page==='many'?'active':''}`} onClick={()=>void navigate('many')}><MessageSquareText size={17}/>多篇分析</button><button className={`nav-item ${page==='issues'?'active':''}`} onClick={()=>void navigate('issues')}><BookOpen size={17}/>问题记录</button></>}
   <div className="sidebar-spacer"/><div className="sidebar-bottom"><button className="nav-item" onClick={()=>void navigate('settings')}><Settings2 size={17}/>数据与外观</button><span>V2 本地写作与 AI 分析</span></div>
  </aside>
  <main className="main" tabIndex={-1}><header className="topbar"><button className="breadcrumb" onClick={()=>void navigate('home')}><LayoutPanelLeft size={17}/>大厅</button><ChevronRight size={14}/><span>{page==='editor'?view?.session.title:({home:'概览',history:'我的作文',questions:'选择题目',import:'题目管理',export:'导出',ai:'模型配置',many:'多篇分析',issues:'问题记录',settings:'数据与外观'}[page])}</span><div className="top-spacer"/><span className="local-label">本地保存</span><button className="icon-button" title="设置" onClick={()=>void navigate('settings')}><Settings2 size={18}/></button></header>
   {!startup?.configured?<section className="setup"><div className="setup-card"><div className="setup-icon"><FolderOpen size={28}/></div><h1>选择写作数据目录</h1><p>题目、作文与 Word 导出所需的版本信息会保存在你能查看和备份的文件夹中。</p><label>推荐位置</label><div className="path-row"><input value={rootChoice} onChange={e=>setRootChoice(e.target.value)}/><button className="secondary" onClick={pickRoot}>浏览…</button></div><button className="primary" onClick={()=>void configure(rootChoice)}>确认并开始</button><small>首次确认后可在“数据与外观”查看或安全迁移。</small></div></section>:
   <div className={`content ${page==='editor'?'content-editor':''}`}>
   {page==='home'&&<Lobby hidden={hideHomeText} onToggleText={()=>setHideHomeText(x=>!x)} notice={setNotice}/>}
   {page==='history'&&<div className="page-wrap"><div className="page-heading"><div><span className="eyebrow">本地归档</span><h1>我的作文</h1><p>打开已保存的写作，继续写作或查看固定版本。</p></div></div>{sessions.length?<div className="session-list">{sessions.map(s=><div key={s.id} className="session-entry"><button className="session-row" onClick={()=>void openSession(s.id)}><div className="file-icon"><FilePenLine size={18}/></div><div><strong>{s.title}</strong><small>{label(s.taskType)} · {s.wordCount} 词 · {s.status==='draft'?'草稿':s.status==='revising'?'更改中':s.status==='editing_original'?'修改初稿中':'初稿已固定'}</small></div><span>{new Date(s.updatedAt).toLocaleDateString('zh-CN')}</span><ChevronRight size={16}/></button><button className="delete-action" aria-label={`删除作文：${s.title}`} title="删除作文" onClick={()=>setDeleteTarget({kind:'session',item:s})}><Trash2 size={17}/><span>删除</span></button></div>)}</div>:<div className="empty">还没有作文。点击左侧“直接开始写作”，输入第一个字符后才会创建文件。</div>}</div>}
   {page==='questions'&&<QuestionBank questions={questions} selected={selectedQuestion} image={questionImg} error={questionError} busy={transitioning} onSelect={q=>void previewQuestion(q)} onImport={()=>void navigate('import')} onDelete={q=>setDeleteTarget({kind:'question',item:q})} onWrite={q=>void createFrom(q)}/>}
   {page==='import'&&<div className="page-wrap narrow"><div className="page-heading"><div><span className="eyebrow">题目管理</span><h1>导入题目图片</h1><p>支持 PNG、JPEG、WebP；导入后复制到数据目录的“题库”。</p></div></div><div className="info-panel"><div className="card-icon yellow"><Upload size={22}/></div><div><h2>本地导入</h2><p>可一次选择多张图片。文件名含 Task2 时会识别为 Task 2，其他默认为 Task 1。</p><button className="primary" onClick={pickImport}>选择图片导入</button></div></div>{importItems.length>0&&<div className="import-review"><h2>确认题目类型</h2>{importItems.map((item,i)=><div className="import-item" key={item.path}><input aria-label={`题目 ${i+1} 标题`} value={item.title} onChange={e=>setImportItems(importItems.map((x,j)=>j===i?{...x,title:e.target.value}:x))}/><select aria-label={`题目 ${i+1} 类型`} value={item.taskType} onChange={e=>setImportItems(importItems.map((x,j)=>j===i?{...x,taskType:e.target.value as 'task1'|'task2'}:x))}><option value="task1">Task 1</option><option value="task2">Task 2</option></select></div>)}<button className="primary" onClick={finishImport}>确认导入 {importItems.length} 张</button></div>}<div className="info-panel muted"><div className="card-icon"><Download size={22}/></div><div><h2>在线题库</h2><p>正式下载来源尚未提供。接入前此处保持空状态，不使用虚构地址。</p><button className="secondary" disabled>暂无可下载题库</button></div></div><p className="subtle">已入库 {questions.length} 张题目图片。原始文件不会因导入而被修改。</p></div>}
   {page==='editor'&&view&&<div className={`workspace workflow-${writerMode}`}>
    {examMode&&<button className="icon-button exam-exit" title="退出写作全屏模式" aria-label="退出写作全屏模式" onClick={()=>void toggleExamMode()}><Minimize2 size={20}/></button>}
    <div className="writer-head">
     <div className="writer-title"><input key={view.session.id+view.session.title} defaultValue={view.session.title} onBlur={e=>void changeTitle(e.target.value)} aria-label="作文标题"/><div><span className="task-pill">{label(view.session.taskType)}</span><span className="mode-label">{writerMode==='review'?'批阅结果':writerMode==='revise'?'修改与 AI 聊天':'写作'}</span></div></div>
     <div className="writer-actions">
      {writerMode!=='review'&&<><button className={`timer-button ${view.timerEnabled?'running':''}`} onClick={toggleTimer} disabled={view.session.id==='pending-quick'}><Clock3 size={16}/>{timeText}</button><button className="secondary small" onClick={()=>setShowQuestion(x=>!x)} disabled={!view.session.questionId}>题目栏</button></>}
      {writerMode==='revise'&&<><button className="secondary small" onClick={()=>void runSavedAction(async()=>setShowEssay(x=>!x))}>文章栏</button><button className="secondary small" onClick={()=>setShowChat(x=>!x)}>AI 栏</button></>}
      {writerMode!=='review'&&<button className="icon-button fullscreen-button" title="进入写作全屏模式" aria-label="进入写作全屏模式" onClick={()=>void toggleExamMode()}><Maximize2 size={19}/></button>}
     </div>
    </div>
    <div className="task-guide">{writerMode==='review'?'点击原文标记或批注卡片可联动定位。点击“修改”进入文章修改与 AI 聊天。':writerMode==='revise'?'文章修改会自动保存。点击“批阅”查看已有批注；需要新的批阅时，点击“重新批阅当前稿”。':`${view.session.taskType==='task1'?'Task 1 · 建议用时 20 分钟，至少 150 词。':view.session.taskType==='task2'?'Task 2 · 建议用时 40 分钟，至少 250 词。':'自由写作 · 不设题目与时间限制。'} 写完后点击“批阅”，完成批阅后即可修改。`}</div>
    <div className="panes">
     {(showQuestion||examMode)&&view.session.questionId&&<><section className="question-pane" style={{width:`${leftWidth}%`}}><div className="pane-head"><strong>题目</strong><button className="icon-button" title="隐藏题目" onClick={()=>setShowQuestion(false)}><X size={16}/></button></div><div className="pane-scroll">{questionImg&&<img src={questionImg} alt="写作题目原图"/>}</div></section><div className="splitter question-splitter" role="separator" aria-label="调整题目栏宽度" onPointerDown={e=>startDrag('left',e)}><span/></div></>}
     {(showEssay||examMode||writerMode==='write')&&<section className="essay-pane"><div className="pane-head"><strong>{writerMode==='revise'?'修改文章':'文章'}</strong><span>{saveState}</span></div>{view.session.status==='revising'&&<div className="ink-toolbar"><span>接下来输入的颜色</span>{(['black','blue','green','red'] as InkColor[]).map(c=><button key={c} title={{black:'黑色',blue:'蓝色',green:'绿色',red:'红色'}[c]} aria-label={{black:'黑色',blue:'蓝色',green:'绿色',red:'红色'}[c]} aria-pressed={ink===c} className={`ink-choice ink-${c} ${ink===c?'selected':''}`} onMouseDown={e=>e.preventDefault()} onClick={()=>setInk(c)}><span/></button>)}</div>}<div className="editor-scroll"><div ref={editor} className="essay-editor" contentEditable={!transitioning&&!submissionPending&&view.session.status!=='original_submitted'} suppressContentEditableWarning spellCheck={false} autoCorrect="off" autoCapitalize="off" {...writingInputAttributes} onBeforeInput={onBeforeInput} onInput={onEdit} onPaste={onPaste} data-placeholder={view.session.status==='original_submitted'?'稿件已固定。点击“批阅”查看结果，或在批阅后点击“修改”。':'在这里开始写作…'} aria-label="作文输入区"/></div><div className="essay-status"><span>{view.session.status==='original_submitted'?'已固定稿件 · 只读':'输入中自动保存'}</span><span>Words: {wordCount(runs)}</span></div></section>}
     {view.session.id!=='pending-quick'&&<><div className="splitter chat-splitter" role="separator" aria-label="调整 AI 栏宽度" onPointerDown={e=>startDrag('right',e)}><span/></div><section className={`chat-pane ${showChat?'':'chat-collapsed'}`} style={{width:`${chatWidth}%`}}><div className="pane-head"><strong>{writerMode==='review'?'批阅':'AI 聊天'}</strong>{writerMode==='revise'&&<button className="icon-button" aria-label="关闭 AI 聊天" onClick={()=>setShowChat(false)}><X size={16}/></button>}</div><EssayWorkspace key={view.session.id} ref={analysis} session={view} mode={writerMode} onState={setReviewState} onModeChange={setWriterMode} notice={setNotice} onConfigure={()=>void navigate('ai')}/></section></>}
    </div>
    <div className="writer-footer">
     <div className="workflow-controls" role="group" aria-label="批阅与修改">
      <button className={writerMode==='revise'?'secondary':'primary'} aria-pressed={writerMode==='review'} onClick={()=>void review()} disabled={transitioning||(view.session.id!=='pending-quick'&&reviewState.loading)||(!reviewState.hasReview&&!runs.some(r=>r.text.trim()))}>{reviewState.busy&&!reviewState.hasReview?'批阅中…':'批阅'}</button>
      <button className={writerMode==='revise'?'primary':'secondary'} aria-pressed={writerMode==='revise'} title={reviewState.hasReview?'进入文章修改与 AI 聊天':'完成批阅后才能修改'} onClick={()=>void revise()} disabled={!reviewState.hasReview||transitioning}>修改</button>
      <span className="footer-note">{reviewState.hasReview?'批阅与修改可随时切换':'完成批阅后可修改文章并与 AI 聊天'}</span>
     </div>
     <div className="workflow-secondary">
      {writerMode==='revise'&&view.session.status==='revising'&&<button className="secondary" onClick={()=>void finish()} disabled={transitioning}>保存修改版本</button>}
      {reviewState.hasReview&&<button className="secondary" onClick={()=>void review(true)} disabled={transitioning||reviewState.busy||reviewState.loading||!reviewState.ready}>重新批阅当前稿</button>}
      <button className="secondary" onClick={()=>void navigate('export')}>导出 Word</button>
     </div>
    </div>
   </div>}
   {page==='export'&&<div className="page-wrap"><div className="page-heading"><div><span className="eyebrow">本地归档</span><h1>导出作文</h1><p>按作文文件夹选择稿件，导出为保留笔迹颜色的 Word 文档。</p></div><button className="primary" onClick={exportFiles}><Download size={16}/>导出所选 Word</button></div>{sessions.length?sessions.map(s=><div className="export-group" key={s.id}><div className="export-head"><FolderOpen size={19}/><div><strong>{s.title}</strong><small>{s.folder}</small></div></div><ExportVersions session={s} selected={exportSelection} setSelected={setExportSelection}/></div>):<div className="empty">还没有可导出的作文。</div>}</div>}
   {page==='ai'&&<ModelSettings notice={setNotice}/>}
   {page==='many'&&<AnalysisWorkspace many={true} notice={setNotice} onConfigure={()=>void navigate('ai')}/>}
   {page==='issues'&&<IssueRegister notice={setNotice}/>}
   {page==='settings'&&<div className="page-wrap narrow"><div className="page-heading"><div><span className="eyebrow">偏好设置</span><h1>数据与外观</h1></div></div><div className="settings-panel"><h2>数据目录</h2><p>{startup.root}</p><div className="button-line"><button className="secondary" onClick={openRoot}><FolderOpen size={16}/>在访达中打开</button><button className="secondary" onClick={pickRoot}>选择迁移目标…</button><button className="secondary" onClick={openTrash} disabled={!api.native}><Trash2 size={16}/>查看回收站</button></div>{rootChoice!==startup.root&&<div className="migration"><input value={rootChoice} onChange={e=>setRootChoice(e.target.value)}/><button className="primary" onClick={()=>void configure(rootChoice,true)}>复制校验并切换</button></div>}<small>请选择空文件夹。保存或 AI 任务进行中时请稍后重试；迁移会复制校验，旧目录保留。</small></div><div className="settings-panel"><h2>外观</h2><div className="segmented">{(['system','light','dark'] as Theme[]).map(t=><button className={theme===t?'selected':''} key={t} onClick={()=>setTheme(t)}>{t==='system'?'跟随系统':t==='light'?<><Sun size={15}/>浅色</>:<><Moon size={15}/>深色</>}</button>)}</div></div></div>}
   </div>}
  </main>{deleteTarget&&<DeleteDialog kind={deleteTarget.kind} title={deleteTarget.item.title} busy={deleting||transitioning} backupHint={api.native?'已删除内容保留在本地回收站，可在“数据与外观”中查看。':'已删除内容保留在此浏览器的本地备份中。'} onCancel={()=>{if(!deleteBusy.current)setDeleteTarget(null)}} onConfirm={()=>void deleteSelected()}/>}{notice&&<div className="toast" role="status">{notice}<button onClick={()=>setNotice('')} aria-label="关闭提示"><X size={15}/></button></div>}
 </div>
}
function ExportVersions({session,selected,setSelected}:{session:SessionSummary;selected:Record<string,boolean>;setSelected:(v:Record<string,boolean>)=>void}){const [versions,setVersions]=useState<SessionView|null>(null);useEffect(()=>{api.call('open_session',{sessionId:session.id}).then(setVersions).catch(()=>{})},[session.id]);return <div className="version-list">{versions?.versions.filter(v=>v.fixed).map(v=>{const key=`${session.id}|${v.id}`;return <label key={v.id}><input type="checkbox" checked={!!selected[key]} onChange={e=>setSelected({...selected,[key]:e.target.checked})}/><span>{v.kind==='original'?(v.path==='versions/original.json'?'初稿第 1 版':`初稿第 ${Number(v.path.match(/original-(\d+)/)?.[1]||2)} 版`):`更改版 ${v.path.match(/\d+/)?.[0]||''}`}</span><small>{v.wordCount} 词 · Word · 保留文字颜色</small></label>})}{!versions?.versions.some(v=>v.fixed)&&<small className="subtle">还没有已固定稿件</small>}</div>}
