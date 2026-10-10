#!/usr/bin/env python3
"""Record paired controller traces without modifying the frozen gameplay code."""
import argparse
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
from baseline import ROOT, digest, run_process


def summarize(out, cases, repeats):
    result = {}
    for case in cases:
        name = case['name']
        count = sum(p['ticks'] for p in case['phases'])
        runs = {engine: [json.loads((out/engine/str(i+1)/(name+'.json')).read_text()) for i in range(repeats)] for engine in ('go','rust')}
        if any(len(r) != count for engine in runs.values() for r in engine):
            raise ValueError('incomplete trace: '+name)
        a,b = runs['go'][0],runs['rust'][0]
        result[name] = {
            'ticks': count,
            'max_horizontal_difference_m':max(math.dist((x['position'][0],x['position'][2]),(y['position'][0],y['position'][2])) for x,y in zip(a,b)),
            'max_vertical_difference_m':max(abs(x['position'][1]-y['position'][1]) for x,y in zip(a,b)),
            'grounded_mismatch_ticks':[x['tick'] for x,y in zip(a,b) if x['grounded'] != y['grounded']],
            'traversal_mode_mismatch_ticks':[x['tick'] for x,y in zip(a,b) if x['mode'] != y['mode']],
            'engines': {engine:{'final_position':rs[0][-1]['position'], 'max_height':max(x['position'][1] for x in rs[0]),
                               'airborne_ticks':sum(not x['grounded'] for x in rs[0]),
                               'max_repeat_position_delta_m':max(math.dist(x['position'],y['position']) for r in rs[1:] for x,y in zip(rs[0],r))}
                        for engine,rs in runs.items()}}
    return result


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('baseline',type=Path)
    p.add_argument('--out',type=Path,required=True)
    p.add_argument('--repeats',type=int,default=3)
    a=p.parse_args()
    if a.repeats<2 or a.out.exists(): p.error('need >=2 repeats and a fresh output directory')
    config=json.loads((ROOT/'tools/migration/movement-inputs.json').read_text())
    if config['hz'] != 60 or config['warmup_ticks'] != 17: p.error('harnesses require 60 Hz and 17 warmup ticks')
    a.out=a.out.resolve(); a.out.mkdir(parents=True)
    source=a.baseline.resolve()/'reference'
    fixture=source/'character/migration_movement_trace_test.go'
    if fixture.exists(): p.error('observer already exists')
    script=ROOT/'tools/migration/movement-inputs.json'
    template=ROOT/'tools/migration/movement_trace_test.go.txt'
    shutil.copy2(script,a.out/'inputs.json')
    shutil.copy2(template,a.out/'go-observer.go.txt')
    env={k:v for k,v in os.environ.items() if not k.startswith('EARTH_TWO_')}
    env['EARTH_TWO_MOVEMENT_INPUT']=str(script)
    metadata={'reference_commit':json.loads((a.baseline/'run.json').read_text())['reference_commit'],
              'rust_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
              'input_sha256':digest(script),'go_observer_sha256':digest(template),
              'rust_observer_sha256':digest(ROOT/'apps/client/tests/character.rs'),'runs':[],
              'scope':'Fixed 60 Hz existing traversal harnesses. Direct intents, not keyboard/render loop or facing animation. Comparisons are observations; no acceptance tolerances invented.'}
    fixture.write_bytes(template.read_bytes())
    try:
        for engine in ('go','rust'):
            for i in range(a.repeats):
                out=a.out/engine/str(i+1);out.mkdir(parents=True)
                cwd=source if engine=='go' else ROOT
                command=['go','test','./character','-run','^TestMigrationMovementTrace$','-count=1','-v'] if engine=='go' else ['cargo','test','--locked','-p','earth-two-client','--no-default-features','--test','character','migration_movement_trace','--','--exact','--nocapture']
                extra={'GOWORK':str(source/'build/deps/native.work'),'GOFLAGS':''} if engine=='go' else {}
                print(engine,i+1,flush=True)
                with (out/'tests.log').open('w') as log:
                    code=run_process(command,cwd=cwd,env=env|extra|{'EARTH_TWO_MOVEMENT_OUT':str(out)},stdout=log,stderr=subprocess.STDOUT,timeout=600)
                metadata['runs'].append({'engine':engine,'repeat':i+1,'exit_code':code,'command':command})
                (a.out/'run.json').write_text(json.dumps(metadata,indent=2)+'\n')
                if code: raise RuntimeError(f'{engine} failed: {out}/tests.log')
    finally:
        fixture.unlink()
    metadata['comparison']=summarize(a.out,json.loads(script.read_text())['cases'],a.repeats)
    (a.out/'run.json').write_text(json.dumps(metadata,indent=2)+'\n')
    print(json.dumps(metadata['comparison'],indent=2))


if __name__=='__main__':main()
