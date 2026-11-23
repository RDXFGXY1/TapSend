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
const api=async(path,options={})=>{const response=await authenticatedFetch(path,options);if(!response.ok)throw new Error(await response.text());return response.json()};

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
 send.onclick=async()=>{if(!note.value.trim())return;send.disabled=true;try{await upload(new File([note.value],`note-${Date.now()}.txt`,{type:'text/plain'}));note.value=''}catch(e){status.textContent=e.message}finally{send.disabled=false}};
 section.ondragover=e=>e.preventDefault();section.ondrop=async e=>{e.preventDefault();try{for(const file of e.dataTransfer.files)await upload(file)}catch(err){status.textContent=err.message}};
}

async function getIdentity(){
  const saved=JSON.parse(localStorage.getItem('tapsend-device')||'null');
  const pairCode=location.hash.slice(1).split('~')[0];
  if(saved && !pairCode)return saved;
  if(!pairCode) throw new Error('This phone is not paired. Scan the pairing QR shown on Linux.');
  const keys=await crypto.subtle.generateKey({name:'Ed25519'},false,['sign','verify']);
  const rawPublic=await crypto.subtle.exportKey('raw',keys.publicKey);
  const id=crypto.randomUUID();
  await api('/api/pair',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({code:pairCode,deviceID:id,name:navigator.platform||'Phone',publicKey:b64url(rawPublic)})});
  // CryptoKey is intentionally not exportable. IndexedDB persists it without a reusable auth cookie.
  await saveKey(keys.privateKey);
  const identity={id};localStorage.setItem('tapsend-device',JSON.stringify(identity));history.replaceState(null,'',location.pathname);return identity;
}
function saveKey(key){return new Promise((resolve,reject)=>{const open=indexedDB.open('tapsend',1);open.onupgradeneeded=()=>open.result.createObjectStore('keys');open.onerror=()=>reject(open.error);open.onsuccess=()=>{const tx=open.result.transaction('keys','readwrite');tx.objectStore('keys').put(key,'device');tx.oncomplete=resolve;tx.onerror=()=>reject(tx.error)}})}
function loadKey(){return new Promise((resolve,reject)=>{const open=indexedDB.open('tapsend',1);open.onerror=()=>reject(open.error);open.onsuccess=()=>{const tx=open.result.transaction('keys');const request=tx.objectStore('keys').get('device');request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error)}})}
async function authenticate(identity){const {challenge}=await api('/api/challenge',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({deviceID:identity.id})});const key=await loadKey();if(!key)throw new Error('The phone key is missing; remove this device on Linux and pair again.');const signature=await crypto.subtle.sign('Ed25519',key,new TextEncoder().encode(challenge));sessionToken=(await api('/api/verify',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({deviceID:identity.id,signature:b64url(signature)})})).token;return sessionToken}
let received=0, receiveSpeed=0;
let activeDownload=null;
function monitor(){let rtt=0;async function beat(){const begin=performance.now();try{await api('/api/heartbeat',{method:'POST',headers:{Authorization:`Bearer ${sessionToken}`,'Content-Type':'application/json'},body:JSON.stringify({rtt,received,speed:receiveSpeed}),signal:AbortSignal.timeout(5000)});rtt=performance.now()-begin;const state=await api('/api/session',{headers:{Authorization:`Bearer ${sessionToken}`}});document.querySelector('#transfer-state').textContent=state.stopped?'Sharing stopped on computer':'Ready to receive';if(state.stopped&&activeDownload)activeDownload.abort();await refreshMessages();await refreshFiles()}catch(error){document.querySelector('#transfer-state').textContent=error.message||'Connection lost'}setTimeout(beat,2000)}beat()}
async function refreshMessages(){const messages=await api('/api/messages',{headers:{Authorization:`Bearer ${sessionToken}`}});const thread=document.querySelector('#thread');if(thread.dataset.last===String(messages.at(-1)?.id||0))return;thread.replaceChildren();for(const message of messages){const bubble=document.createElement('div');bubble.className=`message ${message.from==='phone'?'phone':'computer'}`;bubble.textContent=message.text;thread.append(bubble)}thread.dataset.last=String(messages.at(-1)?.id||0);thread.scrollTop=thread.scrollHeight}
document.querySelector('#message-form').onsubmit=async event=>{event.preventDefault();const input=document.querySelector('#message-text');const text=input.value.trim();if(!text)return;try{await api('/api/messages',{method:'POST',headers:{Authorization:`Bearer ${sessionToken}`,'Content-Type':'application/json'},body:JSON.stringify({text})});input.value='';await refreshMessages()}catch(error){statusEl.textContent=error.message}};
const partialDownloads=new Map();
async function streamToWritable(file,writable,controller){
  const response=await authenticatedFetch(`/api/files/${file.id}`,{headers:{Authorization:`Bearer ${sessionToken}`},signal:controller.signal});
  if(!response.ok)throw new Error(await response.text());
  if(!response.body)throw new Error('This browser cannot stream this download.');
  const reader=response.body.getReader();
  let count=0;const started=performance.now();
  while(true){
    const {done,value}=await reader.read();if(done)break;
    await writable.write(value);count+=value.length;received+=value.length;
    receiveSpeed=count/Math.max((performance.now()-started)/1000,.001);
    statusEl.textContent=`Receiving ${Math.round(count/Math.max(file.size,1)*100)}% · ${(receiveSpeed/1048576).toFixed(2)} MiB/s`;
  }
  if(count!==file.size)throw new Error('Incomplete download. Please retry.');
}
function saveBlob(blob,name){const url=URL.createObjectURL(blob);const link=document.createElement('a');link.href=url;link.download=name;link.click();setTimeout(()=>URL.revokeObjectURL(url),60000)}
async function downloadLarge(file,controller){
  if(window.showSaveFilePicker){
    const handle=await window.showSaveFilePicker({suggestedName:file.name});
    const writable=await handle.createWritable();
    try{await streamToWritable(file,writable,controller);await writable.close()}
    catch(error){await writable.abort().catch(()=>{});throw error}
    return 'Saved on phone';
  }
  const root=await navigator.storage.getDirectory();
  const name=`tapsend-${crypto.randomUUID()}`;
  const handle=await root.getFileHandle(name,{create:true});
  try{
    const writable=await handle.createWritable();
    try{await streamToWritable(file,writable,controller);await writable.close()}
    catch(error){await writable.abort().catch(()=>{});throw error}
    saveBlob(await handle.getFile(),file.name);
    setTimeout(()=>root.removeEntry(name).catch(()=>{}),60000);
    return 'File received · handed to browser for saving';
  }catch(error){await root.removeEntry(name).catch(()=>{});throw error}
}
async function download(file){
  const controller=new AbortController();
  activeDownload=controller;document.querySelector('#cancel-transfer').hidden=false;
  document.querySelector('#cancel-transfer').onclick=()=>controller.abort();
  if(file.size>=32*1048576 && (window.showSaveFilePicker || navigator.storage?.getDirectory)){
    try{
      const result=await downloadLarge(file,controller);
      await api('/api/receipt',{method:'POST',headers:{Authorization:`Bearer ${sessionToken}`,'Content-Type':'application/json'},body:JSON.stringify({id:file.id,state:'received'})}).catch(()=>{});
      return result;
    }finally{receiveSpeed=0;activeDownload=null;document.querySelector('#cancel-transfer').hidden=true}
  }
  const lanes=file.size>=1048576?4:1;
  const chunkSize=lanes===4?Math.min(4*1048576,Math.ceil(file.size/4)):4*1048576;
  const cacheKey=`${file.id}:${file.size}`;
  const parts=partialDownloads.get(cacheKey)||new Array(Math.max(1,Math.ceil(file.size/chunkSize)));
  partialDownloads.set(cacheKey,parts);
  let nextPart=0,count=parts.reduce((n,p)=>n+(p?p.size:0),0),lastPaint=0;
  const started=performance.now();
  async function worker(_,lane){
    while(nextPart<parts.length){
      const index=nextPart++,start=index*chunkSize,end=Math.min(file.size-1,start+chunkSize-1);
      if(parts[index])continue;
      const headers={Authorization:`Bearer ${sessionToken}`,'X-TapSend-Stream':String(lane+1)};
      if(file.size>0)headers.Range=`bytes=${start}-${end}`;
      const response=await authenticatedFetch(`/api/files/${file.id}`,{headers,signal:controller.signal});
      if(!response.ok)throw new Error(await response.text());
      if(file.size>0 && (response.status!==206 || response.headers.get('Content-Range')!==`bytes ${start}-${end}/${file.size}`))throw new Error('Server returned an unexpected file range. Restart sharing and retry.');
      const chunks=[];const reader=response.body.getReader();
      while(true){
        const {done,value}=await reader.read();if(done)break;
        chunks.push(value);count+=value.length;received+=value.length;
        receiveSpeed=count/Math.max((performance.now()-started)/1000,.001);
        if(performance.now()-lastPaint>=200){lastPaint=performance.now();statusEl.textContent=`Receiving ${Math.round(count/Math.max(file.size,1)*100)}% · ${(receiveSpeed/1048576).toFixed(2)} MiB/s · ${lanes} streams`;}
      }
      const blob=new Blob(chunks);
      if(blob.size!==Math.max(0,end-start+1))throw new Error('Incomplete download. Please retry.');
      parts[index]=blob;
      receiveSpeed=count/Math.max((performance.now()-started)/1000,.001);
      if(performance.now()-lastPaint>=200){lastPaint=performance.now();statusEl.textContent=`Receiving ${Math.round(count/Math.max(file.size,1)*100)}% · ${(receiveSpeed/1048576).toFixed(2)} MiB/s · ${lanes} streams`;}
    }
  }
  try{await Promise.all(Array.from({length:lanes},worker));}
  catch(error){controller.abort();if(error.name==='AbortError'||error.name==='TypeError')throw new Error('Download interrupted. Tap Retry to resume.');throw error;}
  finally{receiveSpeed=0;activeDownload=null;document.querySelector('#cancel-transfer').hidden=true;}
  saveBlob(new Blob(parts),file.name);
  partialDownloads.delete(cacheKey);
  await api('/api/receipt',{method:'POST',headers:{Authorization:`Bearer ${sessionToken}`,'Content-Type':'application/json'},body:JSON.stringify({id:file.id,state:'received'})}).catch(()=>{});
  return 'File received · handed to browser for saving';
}
const visibleFiles=new Set();
async function refreshFiles(){
  const files=await api('/api/files',{headers:{Authorization:`Bearer ${sessionToken}`}});
  filesEl.hidden=false;
  for(const file of files){
    if(visibleFiles.has(file.id))continue;
    visibleFiles.add(file.id);
    const row=document.createElement('div');row.className='file';
    const label=document.createElement('span');label.textContent=file.name;
    const size=document.createElement('small');size.textContent=`${(file.size/1048576).toFixed(1)} MiB`;
    label.append(document.createElement('br'),size);
    const button=document.createElement('button');button.textContent='Download ↓';
    button.onclick=async()=>{button.disabled=true;button.textContent='Receiving…';statusEl.textContent=`Receiving ${file.name}`;try{statusEl.textContent=await download(file);button.textContent='Save again ↓'}catch(error){button.textContent='Retry ↓';statusEl.textContent=error.name==='AbortError'?'Download stopped':error.message}finally{button.disabled=false}};
    row.append(label,button);listEl.append(row);
  }
  return files.length;
}
async function start(){try{const identity=await getIdentity();await authenticate(identity);document.querySelector('#messages').hidden=false;enableInbox();try{const count=await refreshFiles();statusEl.textContent=`Device authenticated · ${count} file(s) available`}catch(error){statusEl.textContent=error.message}monitor()}catch(error){statusEl.textContent=error.message}}
start();

