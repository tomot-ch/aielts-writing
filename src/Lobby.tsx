import {useEffect, useRef, useState} from 'react';
import {Eye, EyeOff, ImagePlus, RotateCcw, Scan} from 'lucide-react';
import {clearLobbyImage, readLobbyImage, saveLobbyImage, type LobbyImage} from './lobbyImage';

function checkImage(file: File): Promise<void> {
  if (!/\.(png|jpe?g|webp)$/i.test(file.name) || (file.type && !['image/png', 'image/jpeg', 'image/webp'].includes(file.type))) return Promise.reject(new Error('请选择 PNG、JPG 或 WebP 图片。'));
  if (file.size > 12 * 1024 * 1024) return Promise.reject(new Error('请选择不超过 12 MB 的图片。'));
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const image = new Image();
    image.onload = () => {URL.revokeObjectURL(url); resolve();};
    image.onerror = () => {URL.revokeObjectURL(url); reject(new Error('这张图片无法读取，请换一张图片。'));};
    image.src = url;
  });
}

export function Lobby({hidden, onToggleText, notice}: {hidden: boolean; onToggleText: () => void; notice: (message: string) => void}) {
  const input = useRef<HTMLInputElement>(null);
  const actionBusy = useRef(false);
  const [image, setImage] = useState<LobbyImage | null>(null);
  const [url, setUrl] = useState('');
  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [fit, setFit] = useState<'cover' | 'contain'>(() => localStorage.getItem('ielts-lobby-fit') === 'contain' ? 'contain' : 'cover');
  useEffect(() => {
    let alive = true;
    void readLobbyImage().then(value => {if (alive) setImage(value);}).catch(e => {if (alive) notice(`大厅图片读取失败：${e instanceof Error ? e.message : String(e)}`);}).finally(() => {if (alive) setLoaded(true);});
    return () => {alive = false;};
  }, [notice]);
  useEffect(() => {
    if (!image) {setUrl(''); return;}
    const next = URL.createObjectURL(image.blob);
    setUrl(next);
    return () => URL.revokeObjectURL(next);
  }, [image]);
  useEffect(() => {localStorage.setItem('ielts-lobby-fit', fit);}, [fit]);
  const upload = async (file: File) => {
    if (actionBusy.current) return;
    actionBusy.current = true; setBusy(true);
    try {
      await checkImage(file);
      const next = {blob: file, name: file.name};
      await saveLobbyImage(next);
      setImage(next); notice('大厅图片已保存。');
    } catch (e) {notice(`大厅图片未更换：${e instanceof Error ? e.message : String(e)}`);}
    finally {actionBusy.current = false; setBusy(false);}
  };
  const clear = async () => {
    if (actionBusy.current) return;
    actionBusy.current = true; setBusy(true);
    try {await clearLobbyImage(); setImage(null); notice('已恢复空白大厅。');}
    catch (e) {notice(`大厅图片未清除：${e instanceof Error ? e.message : String(e)}`);}
    finally {actionBusy.current = false; setBusy(false);}
  };
  return <section className={`lobby ${url ? 'has-image' : ''}`} aria-label="大厅" aria-busy={busy}>
    {url && <img className={`lobby-media ${fit}`} src={url} alt={`大厅图片：${image?.name || ''}`}/>}
    <div className="lobby-tools">
      <input ref={input} type="file" accept="image/png,image/jpeg,image/webp" aria-label="大厅图片文件" hidden disabled={busy || !loaded} onChange={e => {const file = e.currentTarget.files?.[0]; e.currentTarget.value = ''; if (file) void upload(file);}}/>
      <button className="icon-button" title={image ? '更换大厅图片' : '上传大厅图片'} aria-label={image ? '更换大厅图片' : '上传大厅图片'} disabled={busy || !loaded} onClick={() => input.current?.click()}><ImagePlus size={18}/></button>
      {image && <><button className="icon-button" title={fit === 'cover' ? '完整显示大厅图片' : '铺满大厅'} aria-label={fit === 'cover' ? '完整显示大厅图片' : '铺满大厅'} disabled={busy} onClick={() => setFit(value => value === 'cover' ? 'contain' : 'cover')}><Scan size={18}/></button><button className="icon-button" title="恢复空白大厅" aria-label="恢复空白大厅" disabled={busy} onClick={() => void clear()}><RotateCcw size={17}/></button></>}
      <button className="icon-button" title={hidden ? '显示大厅文字' : '隐藏大厅文字'} aria-label={hidden ? '显示大厅文字' : '隐藏大厅文字'} onClick={onToggleText}>{hidden ? <Eye size={18}/> : <EyeOff size={18}/>}</button>
    </div>
    {!hidden && <div className="lobby-copy"><h1>让智能辅助你更好的写作</h1><p><span>还原真实雅思机考写作页面，并加入ai进行评阅与修改，</span><span>ai的一切偏好也由你而定</span></p></div>}
  </section>;
}
