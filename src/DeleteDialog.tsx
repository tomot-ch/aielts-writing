import {useEffect, useId, useRef} from 'react';

export function DeleteDialog({kind, title, busy, backupHint, onCancel, onConfirm}: {
  kind: 'session' | 'question';
  title: string;
  busy: boolean;
  backupHint: string;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const titleId = useId();
  const descriptionId = useId();
  const noun = kind === 'session' ? '作文' : '题目';
  useEffect(() => {
    const previous = document.activeElement;
    const element = dialog.current!;
    element.showModal();
    cancel.current?.focus();
    return () => {
      element.close();
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus();
      else document.querySelector<HTMLElement>('main')?.focus();
    };
  }, []);
  return <dialog ref={dialog} className="confirm-dialog delete-dialog" aria-labelledby={titleId}
    aria-describedby={descriptionId} onCancel={event => {event.preventDefault(); if (!busy) onCancel();}}>
    <h2 id={titleId}>删除这{kind === 'session' ? '篇' : '道'}{noun}？</h2>
    <div id={descriptionId}>
      <p className="delete-target">{title}</p>
      <p>{kind === 'session'
        ? '这篇作文的草稿、所有版本、单篇分析和聊天会移入回收站，相关问题记录会从列表移除。已生成的总体分析报告保留。'
        : '题目会从题库移入回收站。已有作文保留独立的题目快照，可以继续写作和分析。'}</p>
      <p>{backupHint}</p>
    </div>
    <div className="button-line">
      <button ref={cancel} className="secondary" disabled={busy} onClick={onCancel}>取消</button>
      <button className="danger" disabled={busy} onClick={onConfirm}>{busy ? '删除中…' : `删除${noun}`}</button>
    </div>
  </dialog>;
}
