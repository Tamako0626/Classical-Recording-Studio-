const types = {html:'text/html',css:'text/css',js:'text/javascript',mjs:'text/javascript'};
const json = (value, status=200) => Response.json(value, {status,headers:{'cache-control':'no-store'}});
export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname === '/api/cover-design') return designCover(request,env);
    if (url.pathname === '/api/work-search') return searchWorks(request);
    if (url.pathname === '/api/workspace') {
      const user = request.headers.get('oai-authenticated-user-id');
      if (!user) return json({error:'请登录 ChatGPT 后使用工作台。'},401);
      const db = env.DB;
      if (!db) return json({error:'数据库尚未连接。'},503);
      if (request.method === 'GET') {
        const row = await db.prepare('SELECT revision, payload FROM studio_workspace WHERE user_id = ?').bind(user).first();
        return json(row ? {revision:row.revision, data:JSON.parse(row.payload)} : {revision:0,data:null});
      }
      if (request.method !== 'PUT') return json({error:'不支持此操作。'},405);
      if (request.headers.get('origin') !== url.origin) return json({error:'请求来源无效。'},403);
      let incoming;
      try { incoming = await request.json(); } catch { return json({error:'请求格式错误。'},400); }
      const {data,revision} = incoming ?? {};
      if (!Number.isSafeInteger(revision) || revision < 0 || !Array.isArray(data?.projects) || !Array.isArray(data?.templates) || !Array.isArray(data?.logs)) return json({error:'工作台数据无效。'},400);
      const payload = JSON.stringify(data);
      if (new TextEncoder().encode(payload).length > 1800000) return json({error:'记录已超出容量，请导出备份并整理历史。'},413);
      const statement = revision === 0
        ? db.prepare('INSERT INTO studio_workspace (user_id, revision, payload) VALUES (?, 1, ?) ON CONFLICT(user_id) DO NOTHING').bind(user,payload)
        : db.prepare('UPDATE studio_workspace SET revision = revision + 1, payload = ? WHERE user_id = ? AND revision = ?').bind(payload,user,revision);
      const result = await statement.run();
      if (!result.meta.changes) return json({error:'其他页面已保存更新。请先备份当前修改，再刷新页面。'},409);
      return json({revision:revision+1});
    }
    const path = url.pathname === '/' ? '/index.html' : url.pathname;
    if (!Object.hasOwn(assets,path)) return new Response('Not found',{status:404});
    const binary=/^\/llm-(?:cmaps|fonts)\//u.test(path);
    const body=binary?Uint8Array.from(atob(assets[path]),char=>char.charCodeAt(0)):assets[path];
    return new Response(body,{headers:{
      'content-type':binary?'application/octet-stream':types[path.split('.').pop()]+'; charset=utf-8',
      'cache-control':'no-cache',
      'x-content-type-options':'nosniff',
      'content-security-policy':"default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; media-src 'self' blob:; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self' https://cdn.jsdelivr.net https://unpkg.com; base-uri 'none'; form-action 'self'"
    }});
  }
};
