const statusEl=document.querySelector('#portal-status');
const portal=document.querySelector('#portal');
const arrivalEl=document.querySelector('#arrival');
const arrivalsEl=document.querySelector('#arrivals');
const linkEl=document.querySelector('#pairing-link');
const qrEl=document.querySelector('#pairing-qr');
const soundButton=document.querySelector('#sound');
let seen=new Set(),started=false,soundOn=false,audioContext;

function colorFor(name){
  const colors=['#7de4dc','#f3a4ee','#f3ce7f','#a9a1ff','#a8eb8d'];
  let hash=0;for(const char of name)hash=(hash*31+char.charCodeAt(0))|0;
  return colors[Math.abs(hash)%colors.length];
}
function keyFor(item){return `${item.time}|${item.name}|${item.bytes}`}
function updatePairing(url){linkEl.href=url;linkEl.textContent=url;qrEl.src=`/api/portal/qr?t=${Date.now()}`}
function row(item){
  const el=document.createElement('div');el.className='arrival-row';el.style.setProperty('--accent',colorFor(item.from||'Phone'));
  const dot=document.createElement('i');const content=document.createElement('div');
  const name=document.createElement('strong');name.textContent=item.name;
  const meta=document.createElement('small');meta.textContent=`${item.from||'Phone'} · ${(item.bytes/1048576).toFixed(1)} MiB`;
  content.append(name,meta);el.append(dot,content);return el;
}
function chime(){
  if(!soundOn)return;
  audioContext ||= new (window.AudioContext||window.webkitAudioContext)();
  const now=audioContext.currentTime;
  for(const [index,note] of [523.25,659.25,783.99].entries()){
    const oscillator=audioContext.createOscillator(),gain=audioContext.createGain();
    oscillator.type='sine';oscillator.frequency.value=note;
    gain.gain.setValueAtTime(0,now+index*.09);gain.gain.linearRampToValueAtTime(.12,now+index*.09+.015);gain.gain.exponentialRampToValueAtTime(.001,now+index*.09+.42);
    oscillator.connect(gain).connect(audioContext.destination);oscillator.start(now+index*.09);oscillator.stop(now+index*.09+.45);
  }
}
function land(item){
  const accent=colorFor(item.from||'Phone');
  portal.style.setProperty('--accent',accent);portal.classList.remove('impact');void portal.offsetWidth;portal.classList.add('impact');
  arrivalEl.replaceChildren();const card=document.createElement('div');card.className='arrival-card';card.style.setProperty('--accent',accent);card.style.setProperty('--glow',`${accent}66`);
  const icon=document.createElement('span');icon.className='arrival-icon';icon.textContent='↘';
  const name=document.createElement('span');name.className='arrival-name';name.textContent=item.name;
  const from=document.createElement('span');from.className='arrival-from';from.textContent=`LANDED FROM ${item.from||'Phone'}`;
  card.append(icon,name,from);arrivalEl.append(card);statusEl.textContent=`${item.name} landed in your inbox`;chime();
  clearTimeout(land.timeout);land.timeout=setTimeout(()=>arrivalEl.replaceChildren(),5000);
}
async function poll(){
  try{
    const response=await fetch('/api/portal',{cache:'no-store'});
    if(!response.ok)throw new Error(response.status===403?'Open this page on the computer.':'Portal connection failed.');
    const data=await response.json();
    if(linkEl.href!==data.pairingURL)updatePairing(data.pairingURL);
    const next=data.arrivals||[];
    if(!started){seen=new Set(next.map(keyFor));started=true}
    else for(const item of next){const key=keyFor(item);if(!seen.has(key)){seen.add(key);land(item)}}
    arrivalsEl.replaceChildren(...next.slice().reverse().map(row));
    if(!next.length){const empty=document.createElement('p');empty.className='empty';empty.textContent='No arrivals yet.';arrivalsEl.append(empty)}
  }catch(error){statusEl.textContent=error.message||'Portal connection lost'}
  setTimeout(poll,1200);
}
qrEl.onerror=()=>{qrEl.hidden=true;document.querySelector('#qr-fallback').hidden=false};
document.querySelector('#refresh').onclick=async()=>{
  const button=document.querySelector('#refresh');button.disabled=true;
  try{const response=await fetch('/api/portal/refresh',{method:'POST'});if(!response.ok)throw new Error(await response.text());updatePairing((await response.json()).pairingURL);qrEl.hidden=false;document.querySelector('#qr-fallback').hidden=true}
  catch(error){statusEl.textContent=error.message}finally{button.disabled=false}
};
document.querySelector('#fullscreen').onclick=()=>document.fullscreenElement?document.exitFullscreen():document.documentElement.requestFullscreen();
soundButton.onclick=()=>{soundOn=!soundOn;soundButton.textContent=soundOn?'Sound on':'Sound off';soundButton.setAttribute('aria-pressed',String(soundOn));if(soundOn){audioContext||=new (window.AudioContext||window.webkitAudioContext)();audioContext.resume()}};
poll();
