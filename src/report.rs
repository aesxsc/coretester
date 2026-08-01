use crate::RunResult;
use std::path::Path;
use std::process::Command;

pub fn render(run: &RunResult) -> String {
    let data = serde_json::to_string(run)
        .expect("serialize report data")
        .replace("</", "<\\/");
    TEMPLATE.replace("__CORETESTER_DATA__", &data)
}

pub fn open(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let status = Command::new("cmd")
        .args(["/C", "start", "", &path.to_string_lossy()])
        .status();
    #[cfg(target_os = "linux")]
    let status = Command::new("xdg-open").arg(path).status();
    #[cfg(target_os = "macos")]
    let status = Command::new("open").arg(path).status();
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    return Err("automatic report opening is unsupported on this OS".into());
    status.map_err(|e| e.to_string()).and_then(|s| {
        if s.success() {
            Ok(())
        } else {
            Err(format!("opener exited with {s}"))
        }
    })
}

const TEMPLATE: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>CoreTester report</title>
<style>
:root{color-scheme:light dark;--bg:#f4f5f7;--surface:#ffffff;--surface2:#e9edf2;--fg:#18202a;--muted:#647181;--border:#cbd3dc;--s1:#176b87;--s2:#2f8f65;--s3:#9a6a18;--bad:#b74343;--good:#19865d;--glow:rgba(23,107,135,.18)}
@media(prefers-color-scheme:dark){:root{--bg:#0b0f14;--surface:#121922;--surface2:#1a2430;--fg:#e7edf5;--muted:#95a3b5;--border:#293646;--s1:#56c7e8;--s2:#65d69e;--s3:#efb85d;--bad:#ff7474;--good:#5cdda0;--glow:rgba(86,199,232,.18)}}
*{box-sizing:border-box}body{margin:0;background:radial-gradient(circle at 80% 0,var(--glow),transparent 32rem),var(--bg);color:var(--fg);font:14px/1.45 system-ui,-apple-system,Segoe UI,sans-serif}main{max-width:1280px;margin:auto;padding:28px}header{display:flex;align-items:flex-end;justify-content:space-between;gap:20px;border-bottom:1px solid var(--border);padding-bottom:20px}h1{font-size:28px;letter-spacing:.06em;margin:0;font-weight:700}h1 span{color:var(--s1)}h2{font-size:16px;margin:0 0 14px;text-transform:uppercase;letter-spacing:.08em}p{margin:4px 0;color:var(--muted)}.meta{text-align:right}.stats{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:12px;margin:18px 0}.stat,.panel{background:color-mix(in srgb,var(--surface) 94%,transparent);border:1px solid var(--border);border-radius:12px}.stat{padding:14px}.stat label{display:block;color:var(--muted);font-size:12px;text-transform:uppercase;letter-spacing:.06em}.stat strong{font-size:25px;font-variant-numeric:tabular-nums}.best{color:var(--good)}.worst{color:var(--bad)}.panel{padding:18px;margin:12px 0}.core-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(82px,1fr));gap:8px}.core{appearance:none;color:var(--fg);background:var(--surface2);border:1px solid transparent;border-radius:9px;padding:10px;text-align:left;cursor:pointer;transition:transform .12s,border-color .12s}.core:hover,.core:focus{border-color:var(--s1);transform:translateY(-1px)}.core[aria-pressed=true]{border-color:var(--s1);box-shadow:0 0 0 2px var(--glow)}.core b{display:block;font-size:16px}.core small{color:var(--muted)}.meter{height:4px;background:var(--border);border-radius:4px;margin-top:8px;overflow:hidden}.meter i{display:block;height:100%;background:var(--s1)}.selection{display:flex;gap:20px;align-items:center;justify-content:space-between;min-height:42px;margin-top:14px;padding-top:12px;border-top:1px solid var(--border)}.selection strong{font-size:18px}.selection-metrics{display:flex;gap:18px;flex-wrap:wrap}.selection-metrics span{white-space:nowrap;color:var(--muted)}.selection-metrics b{color:var(--fg)}.charts{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px}.chart{min-width:0}.chart h3{font-size:13px;margin:0 0 4px;font-weight:600;color:var(--muted)}svg{width:100%;height:auto;display:block}.gridline{stroke:var(--border);stroke-width:1}.axistext{fill:var(--muted);font-size:11px}.bar{fill:var(--s1)}.bar.bestbar{fill:var(--good)}.bar.worstbar{fill:var(--bad)}.dot{fill:var(--s2);stroke:var(--surface);stroke-width:1.5}.trend{stroke:var(--border);stroke-width:1}.table-wrap{overflow:auto}table{width:100%;border-collapse:collapse;font-variant-numeric:tabular-nums}th,td{padding:8px 10px;border-bottom:1px solid var(--border);text-align:right;white-space:nowrap}th{font-size:11px;color:var(--muted);text-transform:uppercase;letter-spacing:.05em}th:first-child,td:first-child{text-align:left;position:sticky;left:0;background:var(--surface)}.heat{display:inline-block;min-width:62px;padding:3px 6px;border-radius:4px;text-align:right;background:color-mix(in srgb,var(--s1) calc(var(--heat)*55%),var(--surface2))}.features{display:flex;flex-wrap:wrap;gap:6px}.chip{padding:4px 7px;border-radius:999px;background:var(--surface2);color:var(--muted);font-size:12px}.chip.used{color:var(--fg);border:1px solid var(--s2)}.note{padding:12px 0 0;color:var(--muted);font-size:12px}.legend{display:flex;gap:14px;color:var(--muted);font-size:12px;margin-bottom:8px}.swatch{display:inline-block;width:9px;height:9px;border-radius:2px;margin-right:4px;background:var(--s1)}.swatch.good{background:var(--good)}.swatch.bad{background:var(--bad)}
@media(max-width:760px){main{padding:16px}.stats,.charts{grid-template-columns:1fr 1fr}header{align-items:flex-start;flex-direction:column}.meta{text-align:left}.selection{align-items:flex-start;flex-direction:column}}@media(max-width:480px){.stats,.charts{grid-template-columns:1fr}}
@media print{body{background:white;color:black}.panel,.stat{break-inside:avoid}}
</style>
</head>
<body><main>
<header><div><h1><span>CORE</span>TESTER</h1><p id="cpuName"></p></div><div class="meta"><div id="platform"></div><p id="runMeta"></p></div></header>
<section class="stats" aria-label="Run highlights">
 <div class="stat"><label>Best logical CPU</label><strong class="best" id="best"></strong></div>
 <div class="stat"><label>Worst logical CPU</label><strong class="worst" id="worst"></strong></div>
 <div class="stat"><label>Score spread</label><strong id="spread"></strong></div>
 <div class="stat"><label>Consistency CV</label><strong id="cv"></strong></div>
</section>
<section class="panel"><h2>Core map</h2><div class="legend"><span><i class="swatch good"></i>best</span><span><i class="swatch"></i>relative score</span><span><i class="swatch bad"></i>worst</span></div><div id="coreGrid" class="core-grid"></div><div id="selection" class="selection" aria-live="polite"></div></section>
<section class="panel"><h2>Performance profile</h2><div id="charts" class="charts"></div></section>
<section class="panel"><h2>Latency vs composite score</h2><svg id="scatter" viewBox="0 0 900 300" role="img" aria-label="Scatterplot of random memory latency against composite score"></svg></section>
<section class="panel"><h2>Hardware acceleration by core</h2><div class="table-wrap"><table id="accel"><thead></thead><tbody></tbody></table></div><p class="note">Values are raw throughput for short instruction-specific loops; compare down a column, not between instructions with different units.</p></section>
<section class="panel"><h2>Detected CPU capabilities</h2><div id="features" class="features"></div><p class="note" id="featureNote"></p></section>
<section class="panel"><h2>Thermal and power telemetry</h2><div id="sensors"></div><p class="note" id="sensorNote"></p></section>
</main>
<script>
const D=__CORETESTER_DATA__;
const $=s=>document.querySelector(s),NS='http://www.w3.org/2000/svg';
const fmt=(v,n=1)=>v==null?'—':Number(v).toFixed(n);
$('#cpuName').textContent=D.identity.name;
$('#platform').textContent=`${D.identity.os} · ${D.identity.logical_cpus} logical CPUs`;
$('#runMeta').textContent=`${new Date(D.timestamp_unix*1000).toLocaleString()} · ${D.config.duration_ms} ms/workload`;
$('#best').textContent=`CPU ${D.summary.best_cpu}`;$('#worst').textContent=`CPU ${D.summary.worst_cpu}`;
$('#spread').textContent=fmt(D.summary.score_spread_percent)+'%';$('#cv').textContent=fmt(D.summary.score_cv_percent,2)+'%';
const minScore=Math.min(...D.cores.map(c=>c.score)),maxScore=Math.max(...D.cores.map(c=>c.score));
function choose(cpu){document.querySelectorAll('.core').forEach(x=>x.setAttribute('aria-pressed',x.dataset.cpu==cpu));const c=D.cores.find(x=>x.cpu==cpu),topology=c.physical_core==null?'':` · physical core ${c.physical_core} · siblings ${c.siblings.join('/')}`;$('#selection').innerHTML=`<strong>CPU ${c.cpu} <small>${c.core_kind}${topology}</small></strong><div class="selection-metrics"><span>Score <b>${fmt(c.score)}</b></span><span>Integer <b>${fmt(c.integer_gops,2)} GOPS</b></span><span>FP <b>${fmt(c.float_gflops,2)} GFLOP/s</b></span><span>Memory <b>${fmt(c.memory_gbps,2)} GB/s</b></span><span>Latency <b>${fmt(c.latency_ns,1)} ns</b></span><span>Stability <b>${fmt(c.stability,1)}%</b></span></div>`}
D.cores.forEach(c=>{const b=document.createElement('button');b.className='core';b.dataset.cpu=c.cpu;b.setAttribute('aria-pressed','false');b.setAttribute('aria-label',`CPU ${c.cpu}, score ${fmt(c.score)}`);const pct=(c.score-minScore)/Math.max(.001,maxScore-minScore)*100,physical=c.physical_core==null?'':` · core ${c.physical_core}`;b.innerHTML=`<b>CPU ${c.cpu}</b><small>${fmt(c.score)} points${physical}</small><div class="meter"><i style="width:${Math.max(5,pct)}%"></i></div>`;if(c.cpu==D.summary.best_cpu)b.style.borderColor='var(--good)';if(c.cpu==D.summary.worst_cpu&&c.cpu!=D.summary.best_cpu)b.style.borderColor='var(--bad)';b.onclick=()=>choose(c.cpu);$('#coreGrid').appendChild(b)});choose(D.summary.best_cpu);
function el(tag,attrs,text){const x=document.createElementNS(NS,tag);for(const[k,v]of Object.entries(attrs||{}))x.setAttribute(k,v);if(text!=null)x.textContent=text;return x}
const metrics=[['Composite score','score','points',false],['Integer throughput','integer_gops','GOPS',false],['Floating point','float_gflops','GFLOP/s',false],['Memory streaming','memory_gbps','GB/s',false],['Random access latency','latency_ns','ns',true],['Stability','stability','%',false]];
metrics.forEach(([title,key,unit,lower])=>{const wrap=document.createElement('div');wrap.className='chart';wrap.innerHTML=`<h3>${title} · ${unit}${lower?' · lower is better':''}</h3>`;const svg=el('svg',{viewBox:'0 0 560 250',role:'img','aria-label':`${title} by logical CPU`});const vals=D.cores.map(c=>c[key]),max=Math.max(...vals)*1.08,min=Math.min(0,...vals);[0,.25,.5,.75,1].forEach(t=>{let y=216-t*190;svg.append(el('line',{x1:42,y1:y,x2:550,y2:y,class:'gridline'}));svg.append(el('text',{x:38,y:y+4,'text-anchor':'end',class:'axistext'},fmt(min+(max-min)*t,unit=='ns'?0:1)))});const bw=Math.max(3,490/D.cores.length*.68),gap=490/D.cores.length;D.cores.forEach((c,i)=>{const h=(c[key]-min)/(max-min)*190,x=51+i*gap,y=216-h;const cls=c.cpu==D.summary.best_cpu?'bar bestbar':c.cpu==D.summary.worst_cpu?'bar worstbar':'bar';const bar=el('rect',{x,y,width:bw,height:h,class:cls,rx:2});bar.append(el('title',{},`CPU ${c.cpu}: ${fmt(c[key],2)} ${unit}`));svg.append(bar);if(D.cores.length<=24)svg.append(el('text',{x:x+bw/2,y:234,'text-anchor':'middle',class:'axistext'},c.cpu))});wrap.append(svg);$('#charts').append(wrap)});
function scatter(){const svg=$('#scatter'),W=900,H=300,p={l:58,r:25,t:16,b:42};const xs=D.cores.map(c=>c.latency_ns),ys=D.cores.map(c=>c.score),xmin=Math.min(...xs)*.96,xmax=Math.max(...xs)*1.04,ymin=Math.min(...ys)*.96,ymax=Math.max(...ys)*1.04;const X=v=>p.l+(v-xmin)/(xmax-xmin)*(W-p.l-p.r),Y=v=>H-p.b-(v-ymin)/(ymax-ymin)*(H-p.t-p.b);[0,.25,.5,.75,1].forEach(t=>{const x=p.l+t*(W-p.l-p.r),y=H-p.b-t*(H-p.t-p.b);svg.append(el('line',{x1:x,y1:p.t,x2:x,y2:H-p.b,class:'gridline'}));svg.append(el('line',{x1:p.l,y1:y,x2:W-p.r,y2:y,class:'gridline'}));svg.append(el('text',{x,y:H-18,'text-anchor':'middle',class:'axistext'},fmt(xmin+t*(xmax-xmin),1)));svg.append(el('text',{x:p.l-7,y:y+4,'text-anchor':'end',class:'axistext'},fmt(ymin+t*(ymax-ymin),0)))});svg.append(el('text',{x:W/2,y:H-3,'text-anchor':'middle',class:'axistext'},'Random access latency (ns)'));svg.append(el('text',{x:14,y:H/2,transform:`rotate(-90 14 ${H/2})`,'text-anchor':'middle',class:'axistext'},'Composite score'));D.cores.forEach(c=>{const g=el('g',{});const dot=el('circle',{cx:X(c.latency_ns),cy:Y(c.score),r:c.cpu==D.summary.best_cpu?7:5,class:'dot'});dot.append(el('title',{},`CPU ${c.cpu}: ${fmt(c.latency_ns,1)} ns, ${fmt(c.score)} points`));g.append(dot);if(D.cores.length<=32)g.append(el('text',{x:X(c.latency_ns)+7,y:Y(c.score)-7,class:'axistext'},c.cpu));svg.append(g)})}scatter();
const accelNames=[...new Set(D.cores.flatMap(c=>Object.keys(c.accel)))];$('#accel thead').innerHTML='<tr><th>Logical CPU</th>'+accelNames.map(n=>`<th>${n}<br><small>${D.cores[0]?.accel_units[n]||''}</small></th>`).join('')+'</tr>';const maxima=Object.fromEntries(accelNames.map(n=>[n,Math.max(...D.cores.map(c=>c.accel[n]||0))]));D.cores.forEach(c=>{$('#accel tbody').insertAdjacentHTML('beforeend','<tr><td>CPU '+c.cpu+'</td>'+accelNames.map(n=>`<td><span class="heat" style="--heat:${(c.accel[n]||0)/maxima[n]}">${fmt(c.accel[n],2)}</span></td>`).join('')+'</tr>')});if(!accelNames.length)$('#accel tbody').innerHTML='<tr><td>No acceleration kernels available on this architecture</td></tr>';
const used=new Set(D.identity.benchmarked_features);D.identity.features.forEach(f=>{const s=document.createElement('span');s.className='chip '+(used.has(f)||[...used].some(u=>u.startsWith(f))?'used':'');s.textContent=f;$('#features').append(s)});$('#featureNote').textContent='Outlined capabilities have a dedicated benchmark path. Detected-but-unoutlined capabilities remain inventoried but are not executed by a dedicated safe kernel.';
const temps=D.cores.filter(c=>c.temperature_after_c!=null),powers=D.cores.filter(c=>c.package_power_w!=null);if(!temps.length&&!powers.length){$('#sensors').textContent='No sensor samples available.'}else{$('#sensors').innerHTML=`${temps.length?`Temperature range: <b>${fmt(Math.min(...temps.map(c=>c.temperature_before_c??c.temperature_after_c)))}–${fmt(Math.max(...temps.map(c=>c.temperature_after_c)))} °C</b>`:''}${temps.length&&powers.length?' · ':''}${powers.length?`Sampled package power: <b>${fmt(Math.min(...powers.map(c=>c.package_power_w)))}–${fmt(Math.max(...powers.map(c=>c.package_power_w)))} W</b>`:''}`}$('#sensorNote').textContent=D.sensor_note;
</script></body></html>"#;
