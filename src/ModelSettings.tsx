import {useEffect, useRef, useState} from 'react';
import {ArrowRight, Check, Circle, Copy, FileText, Image, Link2, LockKeyhole, Plus, SlidersHorizontal, Trash2} from 'lucide-react';
import {api} from './api';
import type {AiPreferences, AnalysisPreset, ConfigDraft, ConfigRef, PromptDraft, PromptRef, CapabilityResult, ContextCapability} from './generated/dto';
const err = (e: unknown) => e instanceof Error ? e.message : String(e);
const initial: ConfigDraft = {id: null, name: '新模型', baseUrl: 'https://api.deepseek.com', apiKey: '', modelName: '', contextLimit: 32000, apiProtocol: 'chat_completions', contextPolicy: 'auto'};
const modelDraft = (c: ConfigRef): ConfigDraft => ({id: c.id, name: c.name, baseUrl: c.baseUrl, modelName: c.modelName, contextLimit: c.contextLimit, apiKey: '', apiProtocol: c.apiProtocol || 'chat_completions', contextPolicy: c.contextPolicy || 'local'});
const presetName = (p: AnalysisPreset) => p.name || `配置${p.slot}`;
const contextSummary = (c?: ConfigRef) => !c ? '模型不可用' : c.contextPolicy === 'remote_required' ? '必须使用服务端接续' : !c.contextPolicy || c.contextPolicy === 'local' ? '本地完整历史' : c.apiProtocol === 'responses' && c.contextCapability?.status === 'supported' ? '自动 · 已验证服务端接续' : '自动 · 使用本地历史';
type Removal = {kind: 'prompt'; prompt: PromptRef} | {kind: 'preset'; preset: AnalysisPreset};

export function ModelSettings({notice}: {notice: (s: string) => void}) {
  const [section, setSection] = useState<'connection' | 'prompts' | 'presets'>('connection');
  const [configs, setConfigs] = useState<ConfigRef[]>([]);
  const [missingKeys, setMissingKeys] = useState<string[]>([]);
  const [prompts, setPrompts] = useState<PromptRef[]>([]);
  const [preferences, setPreferences] = useState<AiPreferences>({schemaVersion: 1, activeSlot: null, presets: []});
  const [loaded, setLoaded] = useState(false);
  const [loadTick, setLoadTick] = useState(0);
  const [draft, setDraft] = useState<ConfigDraft>(initial);
  const [models, setModels] = useState<string[]>([]);
  const [text, setText] = useState<CapabilityResult | null>(null);
  const [vision, setVision] = useState<CapabilityResult | null>(null);
  const [contextCheck, setContextCheck] = useState<ContextCapability | null>(null);
  const [busy, setBusy] = useState('');
  const [error, setError] = useState('');
  const [modelError, setModelError] = useState('');
  const [promptError, setPromptError] = useState('');
  const [modelId, setModelId] = useState('');
  const [promptId, setPromptId] = useState('');
  const [recordIssues, setRecordIssues] = useState(true);
  const [editing, setEditing] = useState<number | null>(null);
  const [names, setNames] = useState<Record<number, string>>({});
  const [pDraft, setPDraft] = useState<PromptDraft>({id: null, name: '', content: ''});
  const [removal, setRemoval] = useState<Removal | null>(null);
  const [removalError, setRemovalError] = useState('');
  const actionBusy = useRef(false);
  const locked = !!busy || !loaded;
  const nextSlot = [1, 2, 3, 4, 5].find(n => !preferences.presets.some(p => p.slot === n));
  const selectedModel = configs.find(c => c.id === modelId);
  const selectedPrompt = prompts.find(p => p.id === pDraft.id);
  const readOnlyPrompt = !!selectedPrompt?.builtIn;
  const affected = removal?.kind === 'prompt' ? preferences.presets.filter(p => p.promptId === removal.prompt.id) : [];
  const active = preferences.presets.find(p => p.slot === preferences.activeSlot);
  const responses = draft.apiProtocol === 'responses';
  const requiresRemote = draft.contextPolicy === 'remote_required';
  const editingPreset = preferences.presets.find(p => p.slot === editing);
  const modelUsable = !!selectedModel?.textCapable && !missingKeys.includes(modelId) && (!!selectedModel.visionCapable || (editingPreset?.analysisConfigId === modelId && !!editingPreset.visionConfigId));
  useEffect(() => {
    if (!api.native) return;
    let alive = true;
    void Promise.all([api.call('ai_configs'), api.call('ai_prompts'), api.call('ai_preferences'), api.call('ai_missing_credentials')])
      .then(([c, p, prefs, missing]) => {
        if (!alive) return;
        setConfigs(c); setPrompts(p); setPreferences(prefs); setMissingKeys(missing);
        const needsKey = c.find(c => missing.includes(c.id));
        if (needsKey) {setDraft(modelDraft(needsKey)); setContextCheck(needsKey.contextCapability);}
        setPDraft(p[0] ? {id: p[0].id, name: p[0].name, content: p[0].content} : {id: null, name: '', content: ''});
        setPromptId(p[0]?.id || ''); setModelId(c.find(c => c.textCapable && c.visionCapable && !missing.includes(c.id))?.id || '');
        setLoaded(true); setError('');
      }).catch(e => {if (alive) setError(err(e));});
    return () => {alive = false;};
  }, [loadTick]);
  const run = async (kind: string, action: () => Promise<void>, fail: (s: string) => void = setError) => {
    if (actionBusy.current) return;
    actionBusy.current = true; setBusy(kind); fail('');
    try {await action();} catch (e) {fail(err(e));} finally {actionBusy.current = false; setBusy('');}
  };
  const change = (patch: Partial<ConfigDraft>) => {
    setDraft(d => ({...d, ...patch}));
    if ('baseUrl' in patch || 'apiKey' in patch || 'modelName' in patch || 'apiProtocol' in patch) {setText(null); setVision(null); setContextCheck(null);}
    if ('baseUrl' in patch || 'apiKey' in patch) setModels([]);
    setModelError('');
  };
  const chooseModel = (id: string) => {
    const c = configs.find(c => c.id === id);
    setDraft(c ? modelDraft(c) : initial); setText(null); setVision(null); setContextCheck(c?.contextCapability || null); setModels([]); setModelError('');
  };
  const test = (image: boolean) => void run(image ? 'vision' : 'text', async () => {
    const result = await api.call('ai_test', {draft, vision: image});
    (image ? setVision : setText)(result);
    if (!result.ok) setModelError(`${image ? '图片' : '文字'}测试未通过：${result.error || '请检查服务商设置'}`);
  }, setModelError);
  const saveModel = () => void run('model', async () => {
    const saved = await api.call('ai_save_config', {draft});
    setConfigs(cs => [...cs.filter(c => c.id !== saved.id), saved]); setDraft(modelDraft(saved)); setModelId(saved.id); setContextCheck(saved.contextCapability);
    setMissingKeys(ids => ids.filter(id => id !== saved.id));
    notice('模型已保存。前往“配置管理”搭配提示词，即可用于分析。');
  }, setModelError);
  const persist = async (next: AiPreferences) => {
    const saved = await api.call('ai_save_preferences', {preferences: next});
    setPreferences(saved); return saved;
  };
  const savePreset = () => void run('preset', async () => {
    const slot = editing ?? nextSlot;
    if (!slot) throw Error('最多保留五个配置，请先删除不需要的配置。');
    const previous = preferences.presets.find(p => p.slot === editing);
    const preset: AnalysisPreset = {slot, name: previous ? presetName(previous) : `配置${slot}`, analysisConfigId: modelId, promptId, recordIssues, visionConfigId: previous?.analysisConfigId === modelId ? previous.visionConfigId : null};
    await persist({...preferences, activeSlot: preferences.activeSlot ?? slot, presets: [...preferences.presets.filter(p => p.slot !== slot), preset]});
    setEditing(null); notice(`${presetName(preset)}已保存${preferences.activeSlot === null ? '并启用' : ''}。`);
  });
  const editPreset = (p: AnalysisPreset) => {
    setSection('presets'); setEditing(p.slot); setModelId(p.analysisConfigId); setPromptId(p.promptId); setRecordIssues(p.recordIssues); setError('');
    requestAnimationFrame(() => document.getElementById('configuration-builder')?.scrollIntoView({behavior: 'instant', block: 'nearest'}));
  };
  const savePrompt = () => void run('prompt', async () => {
    const saved = await api.call('ai_save_prompt', {draft: pDraft});
    setPrompts(ps => [...ps.filter(p => p.id !== saved.id), saved]); setPDraft({id: saved.id, name: saved.name, content: saved.content});
    setPromptId(saved.id); notice('自定义提示词已保存。');
  }, setPromptError);
  const remove = () => void run('remove', async () => {
    if (removal?.kind === 'prompt') {
      await api.call('ai_delete_prompt', {id: removal.prompt.id});
      setPrompts(ps => ps.filter(p => p.id !== removal.prompt.id));
      if (promptId === removal.prompt.id) setPromptId(prompts.find(p => p.builtIn)?.id || '');
      if (pDraft.id === removal.prompt.id) setPDraft({id: null, name: '', content: ''});
    } else if (removal?.kind === 'preset') {
      const presets = preferences.presets.filter(p => p.slot !== removal.preset.slot);
      await persist({...preferences, presets, activeSlot: preferences.activeSlot === removal.preset.slot ? presets[0]?.slot ?? null : preferences.activeSlot});
      setNames(ns => {const next = {...ns}; delete next[removal.preset.slot]; return next;});
      if (editing === removal.preset.slot) setEditing(null);
    }
    setRemoval(null); notice('已删除。');
  }, setRemovalError);
  return <div className="page-wrap ai-page model-settings oil-page oil-models">
    <div className="page-heading oil-heading"><div><h1>语言大模型配置</h1></div><div className="oil-active-config"><span>当前配置</span><strong>{active ? presetName(active) : '未启用'}</strong></div></div>
    <div className="oil-settings-workspace">
      <aside className="oil-settings-rail">
        <div className="oil-settings-tabs" role="tablist" aria-label="模型设置分类" aria-orientation="vertical" onKeyDown={e => {
          const keys = ['connection', 'prompts', 'presets'] as const;
          const index = keys.indexOf(section);
          const next = e.key === 'ArrowDown' || e.key === 'ArrowRight' ? (index + 1) % keys.length : e.key === 'ArrowUp' || e.key === 'ArrowLeft' ? (index + keys.length - 1) % keys.length : e.key === 'Home' ? 0 : e.key === 'End' ? keys.length - 1 : null;
          if (next !== null) {e.preventDefault(); setSection(keys[next]); document.getElementById(`oil-tab-${keys[next]}`)?.focus();}
        }}>
          {([{key: 'connection', title: '模型连接', hint: '添加与测试模型', Icon: Link2}, {key: 'prompts', title: '提示词管理', hint: '管理分析要求', Icon: FileText}, {key: 'presets', title: '配置管理', hint: `${preferences.presets.length} / 5 个常用配置`, Icon: SlidersHorizontal}] as const).map(({key, title, hint, Icon}) => <button key={key} id={`oil-tab-${key}`} role="tab" aria-label={key === 'presets' ? `配置管理 ${preferences.presets.length} / 5` : title} aria-selected={section === key} aria-controls={`oil-panel-${key}`} tabIndex={section === key ? 0 : -1} onClick={() => setSection(key)}><Icon size={20}/><span><strong>{title}</strong><small>{hint}</small></span></button>)}
        </div>
        <p className="oil-rail-note">单篇与多篇分析共用配置</p>
      </aside>
      <div className="oil-settings-content">
        {!api.native && <div className="oil-desktop-hint" role="note">请在桌面应用中连接模型和保存配置。</div>}
        {error && <div className="ai-inline-error" role="alert">{error}{!loaded && <button className="text-button" onClick={() => setLoadTick(n => n + 1)}>重新加载</button>}</div>}
        {missingKeys.length > 0 && <div className="ai-inline-error" role="status">{configs.filter(c => missingKeys.includes(c.id)).map(c => c.name).join('、')}需要重新填写 API 密钥。选择对应模型，重填密钥并完成测试后保存，无需 Mac 登录密码。</div>}
        <section className="oil-connection-panel" id="oil-panel-connection" role="tabpanel" aria-labelledby="oil-tab-connection" hidden={section !== 'connection'}>
          <div className="settings-section-title"><div><h2 id="model-heading">连接模型</h2><p>添加 API 信息，验证文字与图片能力。</p></div><button className="secondary" disabled={locked} onClick={() => {chooseModel(''); requestAnimationFrame(() => document.getElementById('oil-model-name')?.focus());}}><Plus size={15}/>添加模型</button></div>
      <div className="ai-form model-connection-form">
        <label>已有模型<select aria-label="已有模型" value={draft.id || ''} disabled={locked} onChange={e => chooseModel(e.target.value)}><option value="">添加新模型</option>{configs.map(c => <option key={c.id} value={c.id}>{c.name} · {c.modelName}</option>)}</select></label>
        <label>模型备注名称<input id="oil-model-name" value={draft.name} disabled={locked} onChange={e => change({name: e.target.value})}/></label>
        <label>调用方式<select value={draft.apiProtocol || 'chat_completions'} disabled={locked} onChange={e => change({apiProtocol: e.target.value as ConfigDraft['apiProtocol'], ...(e.target.value === 'chat_completions' && requiresRemote ? {contextPolicy: 'auto' as const} : {})})}><option value="chat_completions">普通兼容接口（Chat Completions）</option><option value="responses">Responses 接口</option></select></label>
        <label>上下文管理<select value={draft.contextPolicy || 'local'} disabled={locked} aria-describedby="oil-context-hint" onChange={e => change({contextPolicy: e.target.value as ConfigDraft['contextPolicy']})}><option value="auto">自动选择</option><option value="local">本地完整历史</option><option value="remote_required" disabled={!responses}>必须使用服务端接续</option></select></label>
        <p className="oil-context-hint" id="oil-context-hint">{draft.contextPolicy === 'local' ? '每次附上完整历史。问答保存在本地。' : !responses ? '普通接口使用本地历史。服务端接续需 Responses 接口并通过测试。' : requiresRemote ? '仅使用服务端历史，需通过接续测试；无法接续时会提示错误。' : '验证支持后由服务端接续，否则使用本地历史。两种方式均保存问答。'}</p>
        <label>API 地址<input value={draft.baseUrl} disabled={locked} onChange={e => change({baseUrl: e.target.value})} placeholder="https://服务商地址/v1"/></label>
        <label>API 密钥<input type="password" autoComplete="off" value={draft.apiKey} disabled={locked} onChange={e => change({apiKey: e.target.value})} placeholder={draft.id ? '留空使用已存密钥；失效时请重新填写' : '填写服务商提供的 API Key'}/></label>
        <div className="model-choice"><label htmlFor="model-choice">模型选择</label><div className="model-choice-row"><input id="model-choice" aria-describedby="model-choice-hint" list="available-models" value={draft.modelName} disabled={locked} onChange={e => change({modelName: e.target.value})} placeholder="选择或输入模型名称"/><button className="secondary" type="button" disabled={locked || (!draft.apiKey.trim() && !draft.id) || !draft.baseUrl.trim()} onClick={() => void run('models', async () => {setModels(await api.call('ai_models', {draft}));}, setModelError)}>{busy === 'models' ? '获取中…' : '获取模型列表'}</button></div><datalist id="available-models">{[...new Set([...models, ...configs.filter(c => c.baseUrl === draft.baseUrl).map(c => c.modelName)])].map(m => <option key={m} value={m}/>)}</datalist><small id="model-choice-hint">支持手动输入模型名称。图片测试会发送应用自带的测试图。</small></div>
      </div>
      <details className="model-advanced"><summary>高级设置</summary><label>上下文预算（字符）<input type="number" min="1024" max="1000000" value={draft.contextLimit} disabled={locked} onChange={e => change({contextLimit: Number(e.target.value)})}/></label></details>
      {modelError && <div className="ai-inline-error" role="alert">{modelError}</div>}
          <div className="oil-test-section"><h3>能力测试</h3><div className="capability-tests oil-capabilities" role="status">
            {([{kind: 'text', title: '文字', result: text, Icon: FileText, image: false}, {kind: 'vision', title: '图片', result: vision, Icon: Image, image: true}] as const).map(({kind, title, result, Icon, image}) => <div key={kind}><Icon size={20}/><strong>{title}</strong><small>{result?.ok ? <Check size={13}/> : <Circle size={9}/>} {busy === kind ? '测试中…' : result ? result.ok ? '已通过' : '未通过' : '未测试'}</small><button className="secondary" disabled={locked || !draft.modelName.trim()} onClick={() => test(image)}>{`测试${title}`}</button></div>)}
            <div><Link2 size={20}/><strong>上下文接续</strong><small>{contextCheck?.status === 'supported' && responses ? <Check size={13}/> : <Circle size={9}/>}<span>{busy === 'context' ? '测试中…' : !responses ? '使用本地历史' : contextCheck ? ({supported: '已验证支持', unsupported: requiresRemote ? '不支持接续' : '不支持，使用本地历史', unverified: requiresRemote ? '尚未确认支持' : '未确认，使用本地历史'})[contextCheck.status] : requiresRemote ? '需测试后保存' : '未测试，使用本地历史'}</span></small><button type="button" className="secondary" disabled={locked || !draft.modelName.trim() || !responses} onClick={() => void run('context', async () => {setContextCheck(await api.call('ai_test_context', {draft}));}, setModelError)}>测试接续</button></div>
          </div></div>
          {responses && <p className="settings-hint">接续测试调用两次模型，仅发送随机测试信息。{contextCheck?.reason && <span> {contextCheck.reason}</span>}</p>}
          <div className="oil-model-save"><p>{requiresRemote ? '文字、图片与接续测试均通过后可保存。' : '文字、图片测试通过后可保存。'}</p><button className="primary" disabled={locked || !text?.ok || !vision?.ok || (requiresRemote && contextCheck?.status !== 'supported')} onClick={saveModel}>{busy === 'model' ? '保存中…' : '保存模型'}</button></div>
        </section>
    <section className="settings-panel oil-prompts-panel" id="oil-panel-prompts" role="tabpanel" aria-labelledby="oil-tab-prompts" hidden={section !== 'prompts'}>
      <div className="settings-section-title"><div><h2 id="prompt-heading">提示词管理</h2></div></div>
      <div className="oil-prompt-layout"><div className="oil-prompt-sidebar"><div className="prompt-library ai-list">{prompts.map(p => <div className="prompt-library-row" key={p.id}><button className={pDraft.id === p.id ? 'selected' : ''} disabled={locked} onClick={() => {setPDraft({id: p.id, name: p.name, content: p.content}); setPromptError('');}}><strong>{p.name}{p.builtIn && <span className="prompt-lock"><LockKeyhole size={12}/>默认 · 只读</span>}</strong><small>{p.createdAt.slice(0, 10)}</small></button>{!p.builtIn && <button className="delete-action" aria-label={`删除提示词：${p.name}`} disabled={locked} onClick={() => {setRemovalError(''); setRemoval({kind: 'prompt', prompt: p});}}><Trash2 size={15}/></button>}</div>)}</div>
      <div className="button-line prompt-library-actions"><button className="secondary" disabled={locked} onClick={() => {setPDraft({id: null, name: '', content: ''}); setPromptError('');}}><Plus size={15}/>添加提示词</button>{selectedPrompt && <button className="secondary" disabled={locked} onClick={() => setPDraft({id: null, name: `${selectedPrompt.name}（副本）`, content: selectedPrompt.content})}><Copy size={14}/>复制为自定义</button>}</div>
      </div><div className="oil-prompt-content"><div className="ai-form prompt-editor"><div className="oil-field"><label htmlFor="oil-prompt-name">提示词名称</label><input id="oil-prompt-name" readOnly={readOnlyPrompt} value={pDraft.name} disabled={locked} onChange={e => setPDraft({...pDraft, name: e.target.value})} placeholder="留空自动命名"/></div><div className="oil-field"><label htmlFor="oil-prompt-content">提示词内容</label><textarea id="oil-prompt-content" rows={7} readOnly={readOnlyPrompt} value={pDraft.content} disabled={locked} onChange={e => setPDraft({...pDraft, content: e.target.value})} placeholder="说明分析重点；正文与题目会由应用附上"/></div></div>
      {promptError && <div className="ai-inline-error" role="alert">{promptError}</div>}
      {readOnlyPrompt ? <p className="settings-hint">默认提示词不可直接修改或删除，复制后可自由编辑。</p> : <div className="button-line"><button className="primary" disabled={locked || !pDraft.content.trim()} onClick={savePrompt}>{busy === 'prompt' ? '保存中…' : pDraft.id ? '保存提示词修改' : '保存新提示词'}</button></div>}
      </div></div>
    </section>
    <section className="settings-panel oil-presets-panel" id="oil-panel-presets" role="tabpanel" aria-labelledby="oil-tab-presets" hidden={section !== 'presets'}>
      <div className="settings-section-title"><div><h2 id="configuration-heading">配置管理</h2><p>{active ? '单篇聊天与多篇分析共用，下次发送时生效。' : '保存并启用一个配置，即可直接开始 AI 分析与聊天。'}</p></div></div>
      <div className="oil-preset-workspace"><div>
      {!preferences.presets.length && <div className="oil-presets-empty"><FileText size={28} strokeWidth={1.5}/><h3>还没有保存的配置</h3><p>先连接模型，再搭配提示词，保存后就能在这里启用。</p><button className="secondary" onClick={() => setSection('connection')}>前往模型连接<ArrowRight size={15}/></button></div>}
      <div className="configuration-list">{preferences.presets.map(p => <article className={`configuration-card ${preferences.activeSlot === p.slot ? 'active' : ''}`} key={p.slot}>
        <div className="configuration-card-heading"><strong>{presetName(p)}</strong>{preferences.activeSlot === p.slot && <span className="preset-badge"><Check size={13}/>使用中</span>}</div>
        <p>{configs.find(c => c.id === p.analysisConfigId)?.name || '模型不可用'} · {configs.find(c => c.id === p.analysisConfigId)?.modelName}<br/>{prompts.find(pr => pr.id === p.promptId)?.name || '提示词不可用'}{p.visionConfigId && ` · 图片识别：${configs.find(c => c.id === p.visionConfigId)?.name || '模型不可用'}`}</p>
        <p className="oil-preset-context">上下文：{contextSummary(configs.find(c => c.id === p.analysisConfigId))}</p>
        <div className="configuration-rename"><label>配置名称<input aria-label={`配置${p.slot}名称`} maxLength={60} value={names[p.slot] ?? presetName(p)} disabled={locked} onChange={e => setNames(ns => ({...ns, [p.slot]: e.target.value}))}/></label><button className="secondary" disabled={locked || !(names[p.slot]?.trim()) || names[p.slot]?.trim() === presetName(p)} onClick={() => void run('rename', async () => {await persist({...preferences, presets: preferences.presets.map(x => x.slot === p.slot ? {...x, name: names[p.slot].trim()} : x)}); notice('配置名称已保存。');})}>保存名称</button></div>
        <div className="button-line configuration-card-actions"><button className="primary" disabled={locked || preferences.activeSlot === p.slot || missingKeys.includes(p.analysisConfigId) || (!!p.visionConfigId && missingKeys.includes(p.visionConfigId))} onClick={() => void run('activate', async () => {await persist({...preferences, activeSlot: p.slot}); notice(`已启用${presetName(p)}，下次发送消息时生效。`);})}>{preferences.activeSlot === p.slot ? '已启用' : '启用配置'}</button><button className="secondary" disabled={locked} onClick={() => editPreset(p)}>编辑搭配</button><button className="delete-action" disabled={locked} onClick={() => {setRemovalError(''); setRemoval({kind: 'preset', preset: p});}}><Trash2 size={15}/>删除配置</button></div>
      </article>)}</div>
      </div>
    <section className="settings-panel configuration-builder oil-builder" id="configuration-builder" aria-labelledby="oil-builder-heading">
        <div className="settings-section-title"><div><h2 id="oil-builder-heading">{editingPreset ? `编辑${presetName(editingPreset)}` : '搭配提示词'}</h2><p>将模型与提示词保存为一份配置。</p></div></div>
        <div className="ai-form"><label>已保存模型<select aria-label="已保存模型" value={modelId} disabled={locked} onChange={e => setModelId(e.target.value)}><option value="">请选择模型</option>{configs.map(c => <option key={c.id} value={c.id}>{c.name} · {c.modelName}{!c.visionCapable ? '（图片未通过）' : ''}</option>)}</select></label><label>配置提示词<select aria-label="配置提示词" value={promptId} disabled={locked} onChange={e => setPromptId(e.target.value)}><option value="">请选择提示词</option>{prompts.map(p => <option key={p.id} value={p.id}>{p.name}{p.builtIn ? ' · 默认' : ''}</option>)}</select></label></div>
        <label className="ai-check"><input type="checkbox" checked={recordIssues} disabled={locked} onChange={e => setRecordIssues(e.target.checked)}/>自动记录单篇分析中的问题</label>
        {selectedModel && !modelUsable && <p className="settings-hint">请先为所选模型完成文字、图片测试并保存；密钥失效时需重新填写。</p>}
        <div className="button-line"><button className="primary" disabled={locked || !modelUsable || !promptId || (editing === null && !nextSlot)} onClick={savePreset}>{busy === 'preset' ? '保存中…' : editing !== null ? '保存配置修改' : '保存为配置'}</button>{editing !== null ? <button className="secondary" disabled={locked} onClick={() => setEditing(null)}>取消编辑</button> : <span className="subtle">{nextSlot ? `将保存为“配置${nextSlot}”，可在“配置管理”中改名。` : '已保存五个配置，请先删除不需要的配置。'}</span>}</div>
    </section>
      </div>
    </section>
      </div>
    </div>
    {removal && <RemovalDialog title={removal.kind === 'prompt' ? `删除提示词“${removal.prompt.name}”？` : `删除“${presetName(removal.preset)}”？`} busy={!!busy} error={removalError} blocked={affected.length > 0} onCancel={() => {if (!busy) setRemoval(null);}} onConfirm={remove}>
      {removal.kind === 'prompt' ? affected.length ? `此提示词正被 ${affected.map(presetName).join('、')} 使用。请先在配置管理中更换这些配置的提示词，或删除相关配置。` : '删除后将不再出现在提示词库中，已保存的聊天与分析记录保留。' : preferences.activeSlot === removal.preset.slot ? `模型和提示词仍然保留。${preferences.presets.length > 1 ? '删除后将启用剩余的第一个配置。' : '删除后需要重新保存并启用配置才能发送消息。'}` : '只删除这份搭配，模型、提示词和聊天记录仍然保留。'}
    </RemovalDialog>}
  </div>;
}

function RemovalDialog({title, children, busy, error, blocked, onCancel, onConfirm}: {title: string; children: string; busy: boolean; error: string; blocked: boolean; onCancel: () => void; onConfirm: () => void}) {
  const ref = useRef<HTMLDialogElement>(null);
  const id = 'configuration-removal-heading';
  useEffect(() => {const previous = document.activeElement; const node = ref.current!; node.showModal(); return () => {node.close(); if (previous instanceof HTMLElement && previous.isConnected) previous.focus();};}, []);
  return <dialog ref={ref} className="confirm-dialog delete-dialog" aria-labelledby={id} onCancel={e => {e.preventDefault(); onCancel();}}><h2 id={id}>{title}</h2><p>{children}</p>{error && <p className="ai-inline-error" role="alert">{error}</p>}<div className="button-line"><button className="secondary" disabled={busy} onClick={onCancel} autoFocus>{blocked ? '知道了' : '取消'}</button>{!blocked && <button className="danger" disabled={busy} onClick={onConfirm}>{busy ? '删除中…' : '确认删除'}</button>}</div></dialog>;
}
