import type {SessionView, SelectedVersion, VersionRef} from './generated/dto';

export type WriterMode = 'write' | 'review' | 'revise';
export type EssayReviewState = {loading: boolean; ready: boolean; hasReview: boolean; hasPending: boolean; busy: boolean};
export const emptyReviewState: EssayReviewState = {loading: true, ready: false, hasReview: false, hasPending: false, busy: false};

/** Called only after the editor has drained its saves. Fixed versions are immutable. */
export async function prepareReviewSource(session: SessionView, actions: {
  submitOriginal: (sessionId: string) => Promise<VersionRef>;
  finishRevision: (sessionId: string) => Promise<VersionRef>;
}): Promise<SelectedVersion> {
  const id = session.session.id;
  const status = session.session.status;
  const version = status === 'draft' || status === 'editing_original'
    ? await actions.submitOriginal(id)
    : status === 'revising'
      ? await actions.finishRevision(id)
      : [...session.versions].reverse().find(v => v.fixed);
  if (!version?.fixed || version.sessionId !== id) throw Error('没有可批阅的固定稿件，请重新打开作文后重试。');
  return {sessionId: id, versionId: version.id};
}
