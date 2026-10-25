export type LobbyImage = {blob: Blob; name: string};
const DATABASE = 'ielts-appearance';
const STORE = 'lobby';

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE, 1);
    let abandoned = false;
    request.onupgradeneeded = () => {if (!request.result.objectStoreNames.contains(STORE)) request.result.createObjectStore(STORE);};
    request.onerror = () => reject(request.error || new Error('无法打开本地图片存储'));
    request.onblocked = () => {abandoned = true; reject(new Error('本地图片存储正被其他窗口占用，请稍后重试'));};
    request.onsuccess = () => {
      const database = request.result;
      if (abandoned) {database.close(); return;}
      database.onversionchange = () => database.close();
      resolve(database);
    };
  });
}

async function access<T>(mode: IDBTransactionMode, operation: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const database = await openDatabase();
  return new Promise((resolve, reject) => {
    const transaction = database.transaction(STORE, mode);
    const request = operation(transaction.objectStore(STORE));
    let result: T;
    request.onsuccess = () => {result = request.result;};
    transaction.oncomplete = () => {database.close(); resolve(result);};
    transaction.onabort = transaction.onerror = () => {database.close(); reject(transaction.error || new Error('图片未能保存，请重试'));};
  });
}

export async function readLobbyImage(): Promise<LobbyImage | null> {
  const value = await access<LobbyImage | undefined>('readonly', store => store.get('image'));
  if (!value) return null;
  if (!(value.blob instanceof Blob) || typeof value.name !== 'string') throw new Error('已保存的大厅图片无法读取，请重新选择');
  return value;
}

export async function saveLobbyImage(image: LobbyImage): Promise<void> {
  await access('readwrite', store => store.put(image, 'image'));
}

export async function clearLobbyImage(): Promise<void> {
  await access('readwrite', store => store.delete('image'));
}
