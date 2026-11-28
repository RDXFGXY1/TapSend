const statusEl=document.querySelector('#status');
const filesEl=document.querySelector('#files');
const listEl=document.querySelector('#list');
const b64url=bytes=>btoa(String.fromCharCode(...new Uint8Array(bytes))).replaceAll('+','-').replaceAll('/','_').replaceAll('=','');
let sessionToken='',renewingSession=null;
async function renewSession(){
  if(!renewingSession)renewingSession=(async()=>{
    const identity=JSON.parse(localStorage.getItem('tapsend-device')||'null');
    if(!identity)throw new Error('Phone pairing is missing. Scan a fresh QR on the computer.');
    await authenticate(identity);
    return sessionToken;
  })().finally(()=>{renewingSession=null});
  return renewingSession;
}
async function authenticatedFetch(path,options={}){
  if(!options.headers?.Authorization)return fetch(path,options);
  const headers={...options.headers,Authorization:`Bearer ${sessionToken}`};
  let response=await fetch(path,{...options,headers});
  if(response.status===401){
    if(headers.Authorization===`Bearer ${sessionToken}`)await renewSession();
    response=await fetch(path,{...options,headers:{...headers,Authorization:`Bearer ${sessionToken}`}});
  }
  return response;
}
const api=async(path,options={})=>{const response=await authenticatedFetch(path,options);if(!response.ok)throw Object.assign(new Error((await response.text()).trim()),{status:response.status});return response.json()};

function enableInbox(){
 const section=document.querySelector('#outgoing'),status=document.querySelector('#upload-status'),input=document.querySelector('#upload-files'),note=document.querySelector('#note'),send=document.querySelector('#send-note');
 section.hidden=false;
 async function upload(file){
   status.textContent=`Sending ${file.name}…`;
   const cancel=document.querySelector('#cancel-upload');
   async function sendOnce(){return new Promise((resolve,reject)=>{const xhr=new XMLHttpRequest();cancel.hidden=false;cancel.onclick=()=>xhr.abort();xhr.open('POST','/api/upload');xhr.setRequestHeader('Authorization',`Bearer ${sessionToken}`);xhr.setRequestHeader('X-File-Name',file.name.replace(/[^\x20-\x7e]/g,'_'));xhr.upload.onprogress=e=>{if(e.lengthComputable)status.textContent=`Sending ${file.name}: ${Math.round(e.loaded/e.total*100)}%`};xhr.onload=()=>xhr.status===200?resolve():reject(Object.assign(new Error(xhr.responseText),{status:xhr.status}));xhr.onerror=()=>reject(new Error('Connection lost. Retry the upload.'));xhr.onabort=()=>reject(new Error('Upload stopped'));xhr.onloadend=()=>{cancel.hidden=true;cancel.onclick=null};xhr.send(file)})}
   try{await sendOnce()}catch(error){if(error.status!==401)throw error;await renewSession();await sendOnce()}
   status.textContent=`Saved on computer: ${file.name}`;
 }
 input.onchange=async()=>{input.disabled=true;try{for(const file of input.files)await upload(file)}catch(e){status.textContent=e.message}finally{input.disabled=false}};
 const portalInput=document.querySelector('#portal-files'),tile=document.querySelector('#portal-tile'),portalButton=document.querySelector('#portal-send'),names=document.querySelector('#portal-file-names');
 let staged=[],sending=false,startY=null;
 portalInput.onchange=()=>{staged=Array.from(portalInput.files);names.textContent=staged.length===1?staged[0].name:`${staged.length} files ready to fly`;tile.hidden=!staged.length;portalButton.hidden=!staged.length;status.textContent=staged.length?'Flick the file tile upward to send.':''};
 async function portalSend(){if(sending||!staged.length)return;sending=true;portalButton.disabled=true;tile.classList.add('flying');try{for(const file of staged)await upload(file);status.textContent=`${staged.length} file${staged.length===1?'':'s'} landed on computer`;staged=[];portalInput.value='';tile.hidden=true;portalButton.hidden=true}catch(error){status.textContent=error.message}finally{sending=false;portalButton.disabled=false;setTimeout(()=>tile.classList.remove('flying'),750)}}
 portalButton.onclick=portalSend;tile.onkeydown=event=>{if(event.key==='Enter'||event.key===' '){event.preventDefault();portalSend()}};
 tile.onpointerdown=event=>{startY=event.clientY;tile.setPointerCapture(event.pointerId)};
 tile.onpointerup=event=>{if(startY!==null&&startY-event.clientY>45)portalSend();startY=null};
 send.onclick=async()=>{if(!note.value.trim())return;send.disabled=true;try{await upload(new File([note.value],`note-${Date.now()}.txt`,{type:'text/plain'}));note.value=''}catch(e){status.textContent=e.message}finally{send.disabled=false}};
 section.ondragover=e=>e.preventDefault();section.ondrop=async e=>{e.preventDefault();try{for(const file of e.dataTransfer.files)await upload(file)}catch(err){status.textContent=err.message}};
}

async function getIdentity(){
  const saved=JSON.parse(localStorage.getItem('tapsend-device')||'null');
  const pairCode=location.hash.slice(1).split('~')[0];
  if(saved && !pairCode)return saved;
  if(!pairCode) throw new Error('This phone is not paired. Open pairing in TapSend on your computer and scan its QR.');
  const keys=await crypto.subtle.generateKey({name:'Ed25519'},false,['sign','verify']);
  const rawPublic=await crypto.subtle.exportKey('raw',keys.publicKey);
  const id=crypto.randomUUID();
  const platform=/Android/i.test(navigator.userAgent)?'Android':(/iPhone|iPad/.test(navigator.userAgent)?'Apple mobile':navigator.platform);
  const name=(platform?platform+' browser':'Phone browser')+' · '+id.slice(0,4);
  await api('/api/pair',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({code:pairCode,deviceID:id,name,publicKey:b64url(rawPublic)})});
  // CryptoKey is intentionally not exportable. IndexedDB persists it without a reusable auth cookie.
  await saveKey(keys.privateKey);
  const identity={id};localStorage.setItem('tapsend-device',JSON.stringify(identity));history.replaceState(null,'',location.pathname);return identity;
}
let database;
function openDatabase(){
  if(!database)database=new Promise((resolve,reject)=>{
    const open=indexedDB.open('tapsend',2);
    open.onupgradeneeded=()=>{
      const db=open.result;
      if(!db.objectStoreNames.contains('keys'))db.createObjectStore('keys');
      db.createObjectStore('downloadParts');
    };
    open.onerror=()=>reject(open.error);
    open.onblocked=()=>reject(new Error('Close other TapSend tabs and reopen this page to enable transfer recovery.'));
    open.onsuccess=()=>{
      const db=open.result;
      db.onversionchange=()=>{db.close();database=null};
      resolve(db);
    };
  }).catch(error=>{database=null;throw error});
  return database;
}
async function storageRequest(store,mode,request){
  const db=await openDatabase();
  return new Promise((resolve,reject)=>{
    const tx=db.transaction(store,mode);
    const result=request(tx.objectStore(store));
    tx.oncomplete=()=>resolve(result.result);
    tx.onabort=()=>reject(tx.error || new Error('Browser storage was interrupted.'));
  });
}
function saveKey(key){return storageRequest('keys','readwrite',store=>store.put(key,'device'))}
function loadKey(){return storageRequest('keys','readonly',store=>store.get('device'))}
async function authenticate(identity){const {challenge}=await api('/api/challenge',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({deviceID:identity.id})});const key=await loadKey();if(!key)throw new Error('The phone key is missing; scan a fresh QR on the computer.');const signature=await crypto.subtle.sign('Ed25519',key,new TextEncoder().encode(challenge));sessionToken=(await api('/api/verify',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({deviceID:identity.id,signature:b64url(signature)})})).token;return sessionToken}
let received=0, receiveSpeed=0;
let activeDownload=null;
function monitor(){let rtt=0;async function beat(){const begin=performance.now();try{await api('/api/heartbeat',{method:'POST',headers:{Authorization:`Bearer ${sessionToken}`,'Content-Type':'application/json'},body:JSON.stringify({rtt,received,speed:receiveSpeed}),signal:AbortSignal.timeout(5000)});rtt=performance.now()-begin;const state=await api('/api/session',{headers:{Authorization:`Bearer ${sessionToken}`}});document.querySelector('#transfer-state').textContent=state.stopped?'Sharing stopped on computer':'Ready to receive';if(state.stopped&&activeDownload)activeDownload.abort();renderOffers(state.offers || []);await refreshMessages();await refreshFiles();if(state.scratchpad){document.querySelector('#scratchpad').hidden=false;await refreshScratchpad().catch(()=>{})}}catch(error){document.querySelector('#transfer-state').textContent=error.message||'Connection lost'}setTimeout(beat,2000)}beat()}
async function refreshMessages(){const messages=await api('/api/messages',{headers:{Authorization:`Bearer ${sessionToken}`}});const thread=document.querySelector('#thread');if(thread.dataset.last===String(messages.at(-1)?.id||0))return;thread.replaceChildren();for(const message of messages){const bubble=document.createElement('div');bubble.className=`message ${message.from==='phone'?'phone':'computer'}`;bubble.textContent=message.text;thread.append(bubble)}thread.dataset.last=String(messages.at(-1)?.id||0);thread.scrollTop=thread.scrollHeight}
document.querySelector('#message-form').onsubmit=async event=>{event.preventDefault();const input=document.querySelector('#message-text');const text=input.value.trim();if(!text)return;try{await api('/api/messages',{method:'POST',headers:{Authorization:`Bearer ${sessionToken}`,'Content-Type':'application/json'},body:JSON.stringify({text})});input.value='';await refreshMessages()}catch(error){statusEl.textContent=error.message}};
const scratchpadStatus=document.querySelector('#scratchpad-status');
let scratchpadSnapshot='';
async function refreshScratchpad(){
  const entries=await api('/api/scratchpad',{headers:{Authorization:`Bearer ${sessionToken}`}});
  const snapshot=JSON.stringify(entries);
  if(snapshot===scratchpadSnapshot)return;
  scratchpadSnapshot=snapshot;
  const list=document.querySelector('#scratchpad-items');
  list.replaceChildren();
  if(!entries.length)list.textContent='No snippets yet.';
  for(const entry of entries.reverse()){
    const row=document.createElement('div');
    row.className='file';
    const content=document.createElement('span');
    content.className='snippet-content';
    content.textContent=entry.text;
    const author=document.createElement('small');
    author.textContent=entry.from==='phone'?'You':'Computer';
    content.append(document.createElement('br'),author);
    const copy=document.createElement('button');
    copy.type='button';copy.textContent='Copy';
    copy.onclick=async()=>{
      try{await navigator.clipboard.writeText(entry.text);scratchpadStatus.textContent='Snippet copied.'}
      catch{scratchpadStatus.textContent='Could not copy automatically. Select the text to copy it.'}
    };
    row.append(content,copy);list.append(row);
  }
}
document.querySelector('#scratchpad-form').onsubmit=async event=>{
  event.preventDefault();
  const input=document.querySelector('#snippet-text');
  const button=event.currentTarget.querySelector('button');
  const text=input.value;
  if(!text.trim())return;
  button.disabled=true;
  try{
    await api('/api/scratchpad',{method:'POST',headers:{Authorization:`Bearer ${sessionToken}`,'Content-Type':'application/json'},body:JSON.stringify({text})});
    if(input.value===text)input.value='';
    await refreshScratchpad();
    scratchpadStatus.textContent='Snippet shared with the computer.';
  }catch(error){scratchpadStatus.textContent=error.message}
  finally{button.disabled=false}
};
const partSize=4*1048576;
const partialDownloads=new Map();
const filePrefix=file=>[file.id,file.size];
function partRange(prefix){
  return IDBKeyRange.bound(prefix,[...prefix,[]],false,true);
}
async function storedKeys(prefix){
  return storageRequest('downloadParts','readonly',store=>store.getAllKeys(partRange(prefix))).catch(()=>[]);
}
function saveBlob(blob,name){const url=URL.createObjectURL(blob);const link=document.createElement('a');link.href=url;link.download=name;link.click();setTimeout(()=>URL.revokeObjectURL(url),60000)}
async function finishDownload(file,handle,partCount,readPart,signal){
  const root=!handle && file.size>=32*1048576 && navigator.storage?.getDirectory
    ? await navigator.storage.getDirectory().catch(()=>null) : null;
  const temporary=root && `tapsend-save-${crypto.randomUUID()}`;
  const destination=handle || (root && await root.getFileHandle(temporary,{create:true}));
  if(!destination){
    const parts=[];
    for(let index=0;index<partCount;index++){
      signal.throwIfAborted();
      parts.push(await readPart(index));
    }
    signal.throwIfAborted();
    saveBlob(new Blob(parts),file.name);
    return false;
  }
  let writable;
  let handedOff=false;
  try{
    writable=await destination.createWritable();
    for(let index=0;index<partCount;index++){
      signal.throwIfAborted();
      await writable.write(await readPart(index));
    }
    signal.throwIfAborted();
    await writable.close();
    if(handle)return true;
    saveBlob(await destination.getFile(),file.name);
    handedOff=true;
    return false;
  }catch(error){
    await writable?.abort().catch(()=>{});
    throw error;
  }finally{
    if(temporary){
      const remove=()=>root.removeEntry(temporary).catch(()=>{});
      if(handedOff)setTimeout(remove,60000);
      else await remove();
    }
  }
}
async function download(file){
  if(activeDownload)throw new Error('Finish or stop the current download first.');
  const controller=new AbortController();
  const {signal}=controller;
  const cancel=document.querySelector('#cancel-transfer');
  activeDownload=controller;cancel.hidden=false;cancel.onclick=()=>controller.abort();
  const path=`/api/files/${file.id}`;
  const authorization={Authorization:`Bearer ${sessionToken}`};
  try{
    const handle=file.size>=32*1048576 && window.showSaveFilePicker
      ? await window.showSaveFilePicker({suggestedName:file.name}) : null;
    const info=await authenticatedFetch(path,{method:'HEAD',headers:authorization,signal,cache:'no-store'});
    if(!info.ok)throw new Error('File is no longer available. Ask the computer to share it again.');
    const revision=info.headers.get('ETag');
    if(!revision || info.headers.get('Content-Length')!==String(file.size)){
      throw new Error('The shared file changed. Ask the computer to share it again.');
    }
    const prefix=[...filePrefix(file),revision];
    const memoryKey=JSON.stringify(prefix);
    const memory=partialDownloads.get(memoryKey)||new Map();
    partialDownloads.set(memoryKey,memory);
    const saved=new Set((await storedKeys(prefix)).map(key=>key.at(-1)));
    const partCount=Math.max(1,Math.ceil(file.size/partSize));
    const partLength=index=>Math.min(partSize,file.size-index*partSize);
    const lanes=file.size>=1048576?4:1;
    let count=0,next=0,storageFailed=false, failure;
    for(let index=0;index<partCount;index++){
      if(saved.has(index)||memory.has(index))count+=partLength(index);
    }
    const restored=count,started=performance.now();
    let lastPaint=-Infinity;
    function progress(){
      receiveSpeed=(count-restored)/Math.max((performance.now()-started)/1000,.001);
      if(performance.now()-lastPaint<200)return;
      lastPaint=performance.now();
      statusEl.textContent=`Receiving ${Math.round(count/Math.max(file.size,1)*100)}% · ${(receiveSpeed/1048576).toFixed(2)} MiB/s`;
      if(restored)statusEl.textContent+=` · ${(restored/1048576).toFixed(1)} MiB restored`;
    }
    async function fetchPart(index,lane){
      const start=index*partSize,end=start+partLength(index)-1;
      const headers={...authorization,'If-Match':revision,'X-TapSend-Stream':String(lane+1)};
      if(file.size>0)headers.Range=`bytes=${start}-${end}`;
      const response=await authenticatedFetch(path,{headers,signal,cache:'no-store'});
      if(response.status===412)throw new Error('The shared file changed. Ask the computer to share it again.');
      if(!response.ok)throw new Error((await response.text()).trim());
      if(response.headers.get('ETag')!==revision ||
        (file.size>0 && (response.status!==206 || response.headers.get('Content-Range')!==`bytes ${start}-${end}/${file.size}`))){
        throw new Error('The computer returned an unexpected file range. Retry the download.');
      }
      const buffers=[];
      let length=0;
      const reader=response.body?.getReader();
      while(reader){
        const {done,value}=await reader.read();
        if(done)break;
        length+=value.length;
        if(length>partLength(index))throw new Error('Received more data than expected. Retry the download.');
        buffers.push(value);count+=value.length;received+=value.length;progress();
      }
      if(length!==partLength(index))throw new Error('Incomplete download. Tap Retry to resume.');
      const blob=new Blob(buffers);
      if(!storageFailed){
        try{
          await storageRequest('downloadParts','readwrite',store=>store.put(blob,[...prefix,index]));
          return blob;
        }catch{storageFailed=true}
      }
      memory.set(index,blob);
      return blob;
    }
    progress();
    await Promise.all(Array.from({length:lanes},async(_,lane)=>{
      try{
        while(!signal.aborted && next<partCount){
          const index=next++;
          if(!saved.has(index) && !memory.has(index))await fetchPart(index,lane);
        }
      }catch(error){if(!failure)failure=error;controller.abort()}
    }));
    if(failure)throw failure;
    signal.throwIfAborted();
    statusEl.textContent='Preparing file for saving…';
    const confirmed=await finishDownload(file,handle,partCount,async index=>{
      const blob=memory.get(index)||await storageRequest('downloadParts','readonly',store=>store.get([...prefix,index])).catch(()=>null);
      if(blob?.size===partLength(index))return blob;
      count-=partLength(index);
      return fetchPart(index,0);
    },signal);
    partialDownloads.delete(memoryKey);
    await storageRequest('downloadParts','readwrite',store=>store.delete(partRange(prefix))).catch(()=>{});
    await api('/api/receipt',{method:'POST',headers:{...authorization,'Content-Type':'application/json'},body:JSON.stringify({id:file.id,state:'received',saveUnconfirmed:!confirmed})}).catch(()=>{});
    return confirmed?'Saved on phone':'File received · handed to browser for saving';
  }catch(error){
    if(error.name==='AbortError'||error.name==='TypeError'){
      throw new Error('Download interrupted. Tap Retry to resume.');
    }
    throw error;
  }finally{
    receiveSpeed=0;activeDownload=null;cancel.hidden=true;cancel.onclick=null;
  }
}
let offerSnapshot='';
function renderOffers(offers){
  const snapshot=JSON.stringify(offers);
  if(snapshot===offerSnapshot)return;
  offerSnapshot=snapshot;
  const container=document.querySelector('#file-offers');
  container.replaceChildren();
  for(const offer of offers){
    const row=document.createElement('div');
    row.className='file offer';
    const label=document.createElement('span');
    label.textContent=`Incoming offer · ${offer.files.length} file(s)`;
    const names=document.createElement('small');
    names.textContent=offer.files.map(file=>file.name).join(' · ');
    label.append(document.createElement('br'),names);
    const actions=document.createElement('div');
    actions.className='offer-actions';
    for(const [state,title] of [['accepted','Accept'],['declined','Decline']]){
      const button=document.createElement('button');
      button.type='button';button.textContent=title;
      button.onclick=async()=>{
        for(const control of actions.children)control.disabled=true;
        try{
          await api(`/api/offers/${encodeURIComponent(offer.id)}`,{
            method:'POST',headers:{Authorization:`Bearer ${sessionToken}`,'Content-Type':'application/json'},
            body:JSON.stringify({state})
          });
          row.remove();
          statusEl.textContent=state==='accepted'?'Offer accepted · choose a file to save':'Offer declined';
          if(state==='accepted')await refreshFiles();
        }catch(error){
          statusEl.textContent=error.message;
          for(const control of actions.children)control.disabled=false;
        }
      };
      actions.append(button);
    }
    row.append(label,actions);container.append(row);
  }
}
const visibleFiles=new Map();
async function refreshFiles(){
  const files=await api('/api/files',{headers:{Authorization:`Bearer ${sessionToken}`}});
  filesEl.hidden=false;
  const current=new Set(files.map(file=>file.id));
  for(const [id,row] of visibleFiles){
    if(!current.has(id)){row.remove();visibleFiles.delete(id)}
  }
  for(const file of files){
    if(visibleFiles.has(file.id))continue;
    const row=document.createElement('div');row.className='file';
    const label=document.createElement('span');label.textContent=file.name;
    const size=document.createElement('small');size.textContent=`${(file.size/1048576).toFixed(1)} MiB`;
    label.append(document.createElement('br'),size);
    const button=document.createElement('button');button.textContent='Download ↓';
    button.onclick=async()=>{button.disabled=true;button.textContent='Receiving…';statusEl.textContent=`Receiving ${file.name}`;try{statusEl.textContent=await download(file);button.textContent='Save again ↓'}catch(error){button.textContent='Retry ↓';statusEl.textContent=error.name==='AbortError'?'Download stopped':error.message}finally{button.disabled=false}};
    visibleFiles.set(file.id,row);
    row.append(label,button);listEl.append(row);
    storedKeys(filePrefix(file)).then(keys=>{
      if(keys.length && button.textContent==='Download ↓')button.textContent='Resume ↓';
    });
  }
  return files.length;
}
async function start(){try{const identity=await getIdentity();await authenticate(identity);document.querySelector('#messages').hidden=false;enableInbox();try{const count=await refreshFiles();statusEl.textContent=`Device authenticated · ${count} file(s) available`}catch(error){statusEl.textContent=error.message}monitor()}catch(error){
  const help=document.querySelector('#pairing-help');
  const next=document.querySelector('#pairing-next');
  next.textContent=error.status===401
    ? 'Open pairing on the computer and scan a fresh QR.'
    : error instanceof TypeError
      ? 'Check that both devices are on the same network, then try opening the pairing link again.'
      : error.message;
  statusEl.textContent='Could not connect to TapSend.';
  help.open=true;
}}
start();

