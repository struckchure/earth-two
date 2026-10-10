#!/usr/bin/env python3
"""Render the initial parity evidence as a local HTML comparison report."""
import argparse
import html
import json
from pathlib import Path


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('evidence',type=Path)
    a=p.parse_args()
    sizes=json.loads((a.evidence/'bundle-size.json').read_text())
    movement=json.loads((a.evidence/'movement-fixed/run.json').read_text())['comparison']
    rows=[]
    def row(label,go,rust):
        rows.append(f'<tr><th>{html.escape(label)}</th><td>{go/2**20:.2f}</td><td>{rust/2**20:.2f}</td><td>{(rust/go-1)*100:+.1f}%</td></tr>')
    g,r=sizes['go_web']['totals'],sizes['rust_web']['totals']
    row('Web code + shell, raw',g['runtime']['bytes'],r['runtime']['bytes'])
    row('Web code + shell, gzip-9',g['runtime']['gzip9_bytes'],r['runtime']['gzip9_bytes'])
    row('Full web payload, raw',sum(v['bytes'] for v in g.values()),sum(v['bytes'] for v in r.values()))
    row('Full web payload, gzip-9',sum(v['gzip9_bytes'] for v in g.values()),sum(v['gzip9_bytes'] for v in r.values()))
    row('macOS arm64 executable',sizes['native']['go']['bytes'],sizes['native']['rust']['bytes'])
    row('Native executable + assets, raw',sizes['native']['go']['bytes']+g['assets']['bytes'],sizes['native']['rust']['bytes']+r['assets']['bytes'])
    traces=''.join(f'<tr><th>{html.escape(name)}</th><td>{c["max_horizontal_difference_m"]:.8f} m</td><td>{c["max_vertical_difference_m"]:.5f} m</td><td>{len(c["traversal_mode_mismatch_ticks"])}</td></tr>' for name,c in movement.items())
    page='''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Earth Two — first parity comparison</title>
<style>body{margin:0;background:#151719;color:#eee;font:16px/1.55 system-ui,sans-serif}main{max-width:1500px;margin:auto;padding:32px}h1{font-size:32px}h2{margin-top:36px}p{max-width:1000px;color:#c8cbd0}a{color:#9ac9f8}table{border-collapse:collapse;max-width:100%;font-variant-numeric:tabular-nums}th,td{padding:10px 18px;border-bottom:1px solid #43484e;text-align:right}th:first-child{text-align:left}select{padding:10px;font:inherit;background:#25282d;color:white;border:1px solid #818993;border-radius:6px}.pair{display:grid;grid-template-columns:1fr 1fr;gap:12px}figure{margin:12px 0}img{width:100%;height:auto;display:block}figcaption{font-weight:700;padding:6px}.note{border-left:3px solid #e7a54b;padding:10px 18px;background:#25211c}@media(max-width:800px){.pair{grid-template-columns:1fr}main{padding:16px}th,td{padding:7px;font-size:13px}}</style>
<main><h1>Earth Two: first Go / Rust parity comparison</h1>
<p>Frozen Go <code>c561bd69</code> compared with Rust checkpoint <code>6e9536a</code>. The current working tree also fixes the measured roll/slide timing difference. Local macOS arm64 evidence; this is an initial comparison, not full parity acceptance.</p>
<h2>Matched native views</h2><p>Same assets, 11:00 clear weather, default outfit and 2560 × 1440 captures. The orbit shot exercises the game camera; fixed shots use identical eye/target coordinates. Animation phases are not synchronized. Screenshot FPS counters are not benchmark results.</p>
<label for="view">View </label><select id="view"><option value="gate_orbit">South gate — orbit camera</option><option value="gate_fixed">South gate — fixed camera</option><option value="market_fixed">Hull Market — fixed camera</option></select>
<div class="pair"><figure><figcaption>Go reference</figcaption><a id="go-link" href="go-views/gate_orbit.png"><img id="go" src="go-views/gate_orbit.png" alt="Go reference South gate"></a></figure><figure><figcaption>Rust after timing fix</figcaption><a id="rust-link" href="rust-views-fixed/gate_orbit.png"><img id="rust" src="rust-views-fixed/gate_orbit.png" alt="Rust South gate"></a></figure></div>
<p class="note">Open finding: Hull Market is much more brightly lit in Rust at the same fixed viewpoint. Exterior framing is close, with visible shadow, outline and HUD differences. Still images establish neither moving-cloth nor animation timing acceptance.</p>
<h2>Bundle sizes</h2><p>All values below are MiB (1,048,576 bytes). Packed assets match by relative path, size and SHA-256. Both optimized native executables link only OS libraries/frameworks.</p><table><thead><tr><th>Payload</th><th>Go</th><th>Rust</th><th>Change</th></tr></thead><tbody>SIZE_ROWS</tbody></table>
<p>Gzip values use the same Python gzip-9 encoder and exclude duplicate sidecars and TypeScript declarations. Go compresses one asset data bundle; Rust assets are compressed per file. These full-corpus estimates are not measured startup network traffic. Native rows exclude installer metadata and do not establish installed-app compatibility.</p>
<p class="note">Current packaging differs: Go ships gzip sidecars (121.83 MiB for the complete served payload); Rust currently stages no sidecars. Rust's WASM also retains 43.83 MiB of function names. Those are concrete optimization targets, not savings already applied.</p>
<h2>Movement traces</h2><p>Seven fresh floor scenarios, 60 Hz, three repeats per engine. Direct movement intents use the existing controller harnesses. All repeats were identical within each engine. Jump airtime, grounded states and traversal modes agree after the timestep fix.</p>
<table><thead><tr><th>Scenario</th><th>Max horizontal difference</th><th>Max vertical difference</th><th>Mode mismatch ticks</th></tr></thead><tbody>TRACE_ROWS</tbody></table>
<p>The original Rust step rounded to 16,666,667 ns; Go uses 16,666,666 ns. Matching Go fixes the one-tick roll/slide exits. Rust still rests about 2 cm higher on the fixture floor; that contact-offset difference remains open.</p>
<p><a href="bundle-size.json">Raw size inventory and hashes</a> · <a href="movement/run.json">Before timing fix</a> · <a href="movement-fixed/run.json">After timing fix</a> · <a href="validation.json">Validation record</a></p>
<p>Still pending: moving and obstructed camera routes, synchronized clothing/animation clips, browser visual pairs, interiors/night/storm coverage, matched frame-time benchmarks, live accounts and installed-package acceptance. Go remains the release default.</p></main>
<script>document.getElementById('view').addEventListener('change',event=>{for(const engine of ['go','rust']){const src=(engine==='go'?'go-views/':'rust-views-fixed/')+event.target.value+'.png';document.getElementById(engine).src=src;document.getElementById(engine+'-link').href=src;document.getElementById(engine).alt=engine+' '+event.target.selectedOptions[0].textContent;}});</script></html>'''
    (a.evidence/'index.html').write_text(page.replace('SIZE_ROWS',''.join(rows)).replace('TRACE_ROWS',traces))
    print(a.evidence/'index.html')


if __name__=='__main__':main()
