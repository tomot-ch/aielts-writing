import type {SaveReceipt, TextRun} from './generated/dto';

type Binding = {id: string; revision: number};
type SaveState = '已保存' | '待保存' | '保存中…' | '保存失败';

/** Owns the revision and pending writes for one editor session. */
export class SessionSave {
  private binding: Binding | null = null;
  private runs: TextRun[] = [];
  private dirty = false;
  private pending: Promise<boolean> | null = null;

  constructor(
    private readonly write: (id: string, revision: number, runs: TextRun[]) => Promise<SaveReceipt>,
    private readonly state: (state: SaveState) => void,
    private readonly failed: (error: unknown) => void,
  ) {}

  get hasUnsaved(): boolean { return this.dirty || this.pending !== null; }

  /** Switching documents is only allowed after flush has fully drained. */
  reset(id: string | null, revision: number, runs: TextRun[]): void {
    if (this.hasUnsaved) throw Error('请等待当前作文保存完成后再切换。');
    this.binding = id ? {id, revision} : null;
    this.runs = runs.map(run => ({...run}));
  }

  /** A quick-writing document is created after its first edit. Keep that edit. */
  attach(id: string, revision: number): void {
    if (this.binding || this.pending) throw Error('作文保存会话已绑定。');
    this.binding = {id, revision};
  }

  update(runs: TextRun[]): void {
    this.runs = runs.map(run => ({...run}));
    this.dirty = true;
    this.state('待保存');
  }

  discardEmptyQuick(): void {
    if (this.binding || this.pending || this.runs.some(run => run.text.length > 0)) {
      throw Error('不能丢弃已有正文。');
    }
    this.dirty = false;
  }

  async flush(): Promise<boolean> {
    // Even when dirty is false, a write may still be in flight.
    if (this.pending) return this.pending;
    if (!this.dirty) return true;
    if (!this.binding) return false;
    const task = this.drain();
    this.pending = task;
    try { return await task; }
    finally { if (this.pending === task) this.pending = null; }
  }

  private async drain(): Promise<boolean> {
    const binding = this.binding!;
    while (this.dirty) {
      const payload = this.runs.map(run => ({...run}));
      const revision = binding.revision;
      this.dirty = false;
      this.state('保存中…');
      try {
        const receipt = await this.write(binding.id, revision, payload);
        binding.revision = receipt.revision;
      } catch (error) {
        this.dirty = true;
        this.state('保存失败');
        this.failed(error);
        return false;
      }
    }
    this.state('已保存');
    return true;
  }
}
