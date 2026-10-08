// Networking only. The game draws all identity UI, stores encrypted backups,
// and signs every account action in Go on desktop and web.
import { DbConnection } from './bindings';

const sessions = new Map<number, DbConnection>();
let nextHandle = 1;
let capturePaste = false;
let pasted = '';
globalThis.addEventListener?.('paste', (event: ClipboardEvent) => { if (capturePaste) { pasted = event.clipboardData?.getData('text') ?? ''; event.preventDefault(); } });
const api = {
  capturePaste(active: boolean) { capturePaste = active; if (!active) pasted = ''; },
  takePaste() { const result = pasted; pasted = ''; return result; },
  async connect(host: string, database: string, accountID: string) {
    if (!/^e2_[0-9a-f]{64}$/.test(accountID)) throw new Error('Invalid account ID');
    const http = host.replace(/^ws:/, 'http:').replace(/^wss:/, 'https:').replace(/\/$/, '');
    const response = await fetch(`${http}/v1/database/${encodeURIComponent(database)}/identity`, { signal: AbortSignal.timeout(10_000) });
    if (!response.ok) throw new Error('Database is unavailable');
    const scope = (await response.text()).replaceAll('"', '').trim();
    const tokenKey = `earth-two/transport/${scope}/${accountID}`;
    const conn = await new Promise<DbConnection>((resolve, reject) => {
      let pending: DbConnection;
      const timer = setTimeout(() => { pending.disconnect(); reject(new Error('Connection timed out')); }, 15_000);
      pending = DbConnection.builder().withUri(host).withDatabaseName(database)
        .withToken(localStorage.getItem(tokenKey) ?? undefined)
        .onConnect((c, _identity, token) => { clearTimeout(timer); if (!localStorage.getItem(tokenKey)) localStorage.setItem(tokenKey, token); resolve(c); })
        .onConnectError((_ctx, err) => { clearTimeout(timer); reject(err); }).build();
    });
    try {
      await new Promise<void>((resolve, reject) => {
        const timer = setTimeout(() => { conn.disconnect(); reject(new Error('Subscription timed out')); }, 15_000);
        conn.subscriptionBuilder().onApplied(() => { clearTimeout(timer); resolve(); }).onError(c => { clearTimeout(timer); reject(c.event); })
          .subscribe([`SELECT * FROM account WHERE id = '${accountID}'`, 'SELECT * FROM my_account']);
      });
      const handle = nextHandle++; sessions.set(handle, conn);
      return { handle, database: scope, sender: conn.identity!.toHexString(), connection: conn.connectionId.toHexString() };
    } catch (err) { conn.disconnect(); throw err; }
  },
  close(handle: number) { sessions.get(handle)?.disconnect(); sessions.delete(handle); },
  active(handle: number) { return sessions.get(handle)?.isActive ?? false; },
  revision(handle: number) { return [...(sessions.get(handle)?.db.account.iter() ?? [])][0]?.revision.toString() ?? '0'; },
  details(handle: number) {
    const row = [...(sessions.get(handle)?.db.myAccount.iter() ?? [])][0];
    return row ? { id:row.id, publicKey:row.publicKey, revision:row.revision.toString(), email:row.email,displayName:row.displayName } : null;
  },
  async ping(handle: number, nonce: string) {
    const conn = sessions.get(handle);
    if (!conn?.isActive) throw new Error('Account disconnected');
    let timer: ReturnType<typeof setTimeout>;
    try { const result = await Promise.race([conn.procedures.connectionPing({nonce:BigInt(nonce)}), new Promise<never>((_,reject) => { timer=setTimeout(()=>reject(new Error('Latency probe timed out')),2000); })]); return result.toString(); }
    finally { clearTimeout(timer!); }
  },
  async mutate(handle: number, action: string, email: string, proof: Uint8Array) {
    const conn = sessions.get(handle);
    if (!conn?.isActive) throw new Error('Account disconnected');
    const operation = action === 'account.register' ? conn.reducers.registerAccount({proof}) :
      action === 'account.link' ? conn.reducers.linkAccount({proof}) :
      action === 'account.set_email' ? conn.reducers.setAccountEmail({email,proof}) :
      action === 'account.set_display_name' ? conn.reducers.setAccountDisplayName({displayName:email,proof}) : Promise.reject(new Error('Unknown account action'));
    let timer: ReturnType<typeof setTimeout>;
    try { await Promise.race([operation, new Promise<void>((_, reject) => { timer = setTimeout(() => reject(new Error('Account action timed out; reconnect before retrying')), 15_000); })]); }
    finally { clearTimeout(timer!); }
  },
};
Object.assign(globalThis, { EarthTwoAccount: api });
