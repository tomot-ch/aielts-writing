import { invoke } from '@tauri-apps/api/core';
import {wordCount as count} from './wordCount';
import type {CommandName, CommandArguments, CommandResult} from './commands';
export type {InkColor,TextRun,Question,VersionRef,SessionSummary,SessionView,ImportResult,Startup} from './generated/dto';
import type {TextRun,Question,VersionRef,SessionSummary,SessionView} from './generated/dto';
const isNative = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
const KEY='ielts-browser-prototype';
type DeletedSession={view:SessionView;question:Question|null;versionRuns:Record<string,TextRun[]>};
type Demo = {root:string|null;sessions:SessionSummary[];views:Record<string,SessionView>;questions:Question[];versionRuns:Record<string,TextRun[]>;snapshots:Record<string,Question>;trash:{sessions:Record<string,DeletedSession>;questions:Question[]}};
function demo():Demo {try{const d=JSON.parse(localStorage.getItem(KEY)||'') as Demo;return {...d,versionRuns:d.versionRuns||{},snapshots:d.snapshots||{},trash:d.trash||{sessions:{},questions:[]}}}catch{return{root:null,sessions:[],views:{},questions:[],versionRuns:{},snapshots:{},trash:{sessions:{},questions:[]}}}}
function demoImage(q:Question|undefined){if(!q||!q.imageFile.startsWith('data:image/'))throw Error('浏览器原型没有这张图片，请在桌面应用中操作。');return q.imageFile}
function put(d:Demo){localStorage.setItem(KEY,JSON.stringify(d))}
function uid(p:string){return `${p}-${crypto.randomUUID()}`}
async function mock(command:CommandName,args:Record<string,unknown>):Promise<unknown>{
 const d=demo();let value:unknown;
 switch(command){
 case 'startup':value={configured:!!d.root,root:d.root,recommendedRoot:'浏览器原型数据（仅此浏览器）'};break;
 case 'set_data_root':d.root=String(args.path);value=d.root;break;
 case 'list_sessions':value=d.sessions;break;
 case 'list_questions':value=d.questions;break;
 case 'question_image':value=demoImage(d.questions.find(q=>q.id===args.questionId));break;
 case 'session_question_image':value=demoImage(d.snapshots[String(args.sessionId)]);break;
 case 'delete_question':{const id=String(args.questionId);const q=d.questions.find(q=>q.id===id);if(!q)throw Error('题目不存在');d.trash.questions.push(q);d.questions=d.questions.filter(q=>q.id!==id);value=null;break;}
 case 'delete_session':{const id=String(args.sessionId);const v=d.views[id];if(!v)throw Error('作文不存在');const versions:Record<string,TextRun[]>={};for(const version of v.versions){if(d.versionRuns[version.id])versions[version.id]=d.versionRuns[version.id];delete d.versionRuns[version.id]}d.trash.sessions[id]={view:v,question:d.snapshots[id]||null,versionRuns:versions};delete d.views[id];delete d.snapshots[id];d.sessions=d.sessions.filter(s=>s.id!==id);value=null;break;}
 case 'seed_questions':case 'import_images':value={imported:[],skipped:[]};break;
 case 'create_quick':case 'create_from_question':{
  const q=command==='create_from_question'?d.questions.find(q=>q.id===args.questionId):undefined;if(command==='create_from_question'&&!q)throw Error('题目不存在');
  const date=new Date().toISOString().slice(0,10),prefix=`${date} 快速写作 `;const next=Math.max(0,...d.sessions.map(s=>s.title.startsWith(prefix)?Number(s.title.slice(prefix.length))||0:0))+1;
  const title=String(args.title||(q?`${date} ${q.title}`:`${prefix}${next}`));const id=uid('essay');const s:SessionSummary={id,title,questionId:q?.id||null,taskType:q?.taskType||null,status:'draft',wordCount:0,updatedAt:new Date().toISOString(),folder:title};if(q)d.snapshots[id]=structuredClone(q);
  const v:SessionView={session:s,activeRuns:[],saveRevision:0,versions:[],timerEnabled:false,remainingSeconds:null,workingVersionId:null};d.sessions.unshift(s);d.views[id]=v;value=s;break;
 }
 case 'open_session':value=d.views[String(args.sessionId)];break;
 case 'save_session':{const v=d.views[String(args.sessionId)];if(v.saveRevision!==args.expectedRevision)throw Error('版本冲突');v.activeRuns=args.runs as TextRun[];v.saveRevision++;v.session.wordCount=count(v.activeRuns);v.session.updatedAt=new Date().toISOString();value={revision:v.saveRevision,savedAt:v.session.updatedAt,wordCount:v.session.wordCount};break;}
 case 'submit_original':{const v=d.views[String(args.sessionId)];if(v.session.status!=='draft'&&v.session.status!=='editing_original')throw Error('当前不在初稿编辑状态');const previous=[...v.versions].reverse().find(x=>x.kind==='original');const n=v.versions.filter(x=>x.kind==='original').length+1;const r:VersionRef={id:uid('ver-original'),sessionId:v.session.id,kind:'original',sourceVersionId:previous?.id||null,createdAt:new Date().toISOString(),wordCount:count(v.activeRuns),path:n===1?'versions/original.json':`versions/original-${String(n).padStart(3,'0')}.json`,fixed:true};v.versions.push(r);d.versionRuns[r.id]=v.activeRuns.map(x=>({...x}));v.session.status='original_submitted';v.saveRevision++;value=r;break;}
 case 'begin_original_edit':{const v=d.views[String(args.sessionId)];if(v.session.status!=='original_submitted')throw Error('请先完成当前编辑');const previous=[...v.versions].reverse().find(x=>x.kind==='original');if(!previous)throw Error('尚无固定初稿');v.activeRuns=(d.versionRuns[previous.id]||v.activeRuns).map(x=>({...x}));v.session.status='editing_original';v.saveRevision++;value=null;break;}
 case 'begin_revision':{const v=d.views[String(args.sessionId)];if(v.session.status!=='original_submitted')throw Error('请先完成当前初稿编辑');const source=String(args.sourceVersionId);v.activeRuns=(d.versionRuns[source]||v.activeRuns).map(x=>({...x}));const r:VersionRef={id:uid('ver-rev'),sessionId:v.session.id,kind:'revision',sourceVersionId:source,createdAt:new Date().toISOString(),wordCount:count(v.activeRuns),path:`versions/revision-${v.versions.length}.json`,fixed:false};v.versions.push(r);v.workingVersionId=r.id;v.session.status='revising';v.saveRevision++;value=r;break;}
 case 'finish_revision':{const v=d.views[String(args.sessionId)];const r=v.versions.find(x=>x.id===v.workingVersionId);if(!r)throw Error('无更改版');r.fixed=true;d.versionRuns[r.id]=v.activeRuns.map(x=>({...x}));v.workingVersionId=null;v.session.status='original_submitted';v.saveRevision++;value=r;break;}
 case 'rename_session':{const v=d.views[String(args.sessionId)];v.session.title=String(args.title);value=v.session;break;}
 case 'set_timer':{const v=d.views[String(args.sessionId)];v.timerEnabled=Boolean(args.enabled);v.remainingSeconds=v.timerEnabled?2400:null;value=v;break;}
 default:throw Error(`浏览器原型暂不支持 ${command}`);
 }
 d.sessions=d.sessions.map(s=>d.views[s.id]?.session||s);put(d);return value;
}
export const api={
 call:<C extends CommandName>(command:C,...params:CommandArguments<C>):Promise<CommandResult<C>>=>{
  const args=params[0] as Record<string,unknown>|undefined;
  return isNative?invoke<CommandResult<C>>(command,args):mock(command,args||{}) as Promise<CommandResult<C>>;
 },
 native:isNative
};
