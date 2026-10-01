#!/usr/bin/env python3
"""Reproducible native many-pane measurements (Linux/X11 + inspection build).

Frame CPU duration and command-to-file latency are reported separately from
parser throughput. Each case owns its state, endpoint and process. Results are evidence,
not a universal performance claim; run an optimized inspection build for timing.
"""
from __future__ import annotations
import argparse
from datetime import datetime, timezone
import json
import hashlib
import os
from pathlib import Path
import platform
import shlex
import subprocess
import sys
import time
import uuid
import importlib.util
import math

REPO=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location("pace_native_harness",REPO/"scripts/native-harness.py")
harness=importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)

def percentile(values, percentage):
    ordered=sorted(values)
    return ordered[max(0,min(len(ordered)-1,math.ceil(len(ordered)*percentage)-1))] if ordered else None

def tree_nodes(client,address):
    result=harness.inspect(client,address,"tree")["Tree"]["accesskit"]
    return [node for _,node in result["nodes"]]

def pane_nodes(client,address):
    return [node for node in tree_nodes(client,address) if node["properties"].get("label","").startswith("Terminal pane ")]

def click(client,address,node):
    b=node["properties"]["bounds"]
    harness.inspect(client,address,"click",str((b["x0"]+b["x1"])/2),str((b["y0"]+b["y1"])/2))

def frames(log):
    found=[]
    for line in log.read_text(errors="replace").splitlines():
        try: event=json.loads(line)
        except ValueError: continue
        if event.get("operation")=="frames": found.append(event)
    return found

def until(condition,timeout,label):
    deadline=time.monotonic()+timeout
    while time.monotonic()<deadline:
        value=condition()
        if value:return value
        time.sleep(.025)
    raise RuntimeError(f"Timed out waiting for {label}")

def save_layout(panes,split_counter,depth=0):
    if len(panes)==1:return {"kind":"pane","pane":panes[0]["id"]}
    split_counter[0]+=1;ident=split_counter[0];middle=len(panes)//2
    return {"kind":"split","id":ident,"axis":"vertical" if depth%2==0 else "horizontal","ratio":.5,"first":save_layout(panes[:middle],split_counter,depth+1),"second":save_layout(panes[middle:],split_counter,depth+1)}

def make_state(data,count):
    workspaces=[];counter=[0]
    for first in range(1,count+1,8):
        panes=[{"id":id,"cwd":str(data)} for id in range(first,min(first+8,count+1))]
        workspaces.append({"id":len(workspaces)+1,"name":f"Scale {len(workspaces)+1}","cwd":str(data),"panes":panes,"active":panes[-1]["id"],"layout":save_layout(panes,counter)})
    return {"version":1,"active":1,"sidebar":True,"workspaces":workspaces}

def command(client,address,text):
    harness.inspect(client,address,"key","ctrl+u")
    harness.inspect(client,address,"text",text)
    harness.inspect(client,address,"key","Enter")

def start_output(client,address,duration,marker):
    # Bound producer lifetime even if cleanup must forcibly stop the app.
    program=f"import os,time,pathlib;pathlib.Path({str(marker)!r}).write_text('running');end=time.monotonic()+{duration!r};data=b'SCALE_OUTPUT '+b'x'*100+b'\\n';\nwhile time.monotonic()<end:\n os.write(1,data*16);time.sleep(.015)"
    command(client,address,f"python3 -c {shlex.quote(program)} &")
    until(marker.exists,10,"output producer startup marker")

def proc_sample(pid):
    status=Path(f"/proc/{pid}/status").read_text()
    stat=Path(f"/proc/{pid}/stat").read_text().split(') ',1)[1].split()
    rss=next(int(line.split()[1])*1024 for line in status.splitlines() if line.startswith("VmRSS:"))
    return {"cpu_ticks":int(stat[11])+int(stat[12]),"rss_bytes":rss,"threads":next(int(line.split()[1]) for line in status.splitlines() if line.startswith("Threads:"))}

def run_case(args,count,scenario,root):
    output=root/f"{count}-{scenario}";data=output/"data";data.mkdir(parents=True)
    if scenario=="hidden-output" and count==1:
        result={"panes":count,"scenario":scenario,"status":"not-applicable","reason":"One pane has no independent hidden pane"}
        (output/"result.json").write_text(json.dumps(result,indent=2)+"\n");return result
    (data/"workspaces.json").write_text(json.dumps(make_state(data,count)))
    (data/"config.toml").write_text('shell="/bin/sh"\nscrollback=1000\ncursor_blink=false\nconfirm_close=false\n')
    address=harness.free_endpoint();env=os.environ.copy();env.pop("WAYLAND_DISPLAY",None);env["EGUI_INSPECTION"]=address
    log=output/"app.log";result={"panes":count,"scenario":scenario,"endpoint":address,"data_root":str(data),"status":"running","output_producers":[]}
    process=None
    try:
        with log.open('w') as sink:
            started=time.monotonic();process=subprocess.Popen([str(args.app),"--data-root",str(data),"--diagnostics","--size","1180x760"],env=env,stdout=sink,stderr=subprocess.STDOUT,start_new_session=True)
            harness.wait_ready(process,args.client,address,30)
            until(lambda:frames(log) and frames(log)[-1]["resources"]["running"]==count,45,"all pane startups")
            result["startup_seconds"]=time.monotonic()-started
            panes=pane_nodes(args.client,address)
            # Keep every producer alive through eight sequential setups and
            # latency probes, while still bounding a forcibly interrupted run.
            lifetime=args.seconds+600
            def produce(pane):
                label=pane["properties"]["label"];ident=int(label.rsplit(' ',1)[1])
                prior=next(item["parsed"] for item in frames(log)[-1]["panes"] if item["pane"]==ident)
                click(args.client,address,pane)
                marker=data/f"producer-{ident}";start_output(args.client,address,lifetime,marker)
                until(lambda:any(item["pane"]==ident and item["parsed"]>=prior+10_000 for item in frames(log)[-1]["panes"]),10,"verified producer bytes")
                result["output_producers"].append(label)
            if scenario=="visible-output":
                produce(panes[-1])
            elif scenario=="several-output":
                for pane in panes:produce(pane)
            elif scenario=="hidden-output":
                produce(panes[-1])
                if count>8:harness.inspect(args.client,address,"key","ctrl+Tab")
                elif count>1:
                    click(args.client,address,panes[0]);harness.inspect(args.client,address,"key","ctrl+shift+Enter")
            elif scenario=="lifecycle":
                for _ in range(6):
                    if count==64:
                        harness.inspect(args.client,address,"key","ctrl+shift+w")
                        until(lambda:frames(log) and frames(log)[-1]["resources"]["running"]==count-1 and frames(log)[-1]["resources"]["closing"]==0,30,"closed slot teardown")
                        harness.inspect(args.client,address,"key","ctrl+shift+d")
                    else:
                        harness.inspect(args.client,address,"key","ctrl+shift+d")
                        until(lambda:frames(log) and frames(log)[-1]["resources"]["running"]==count+1 and frames(log)[-1]["resources"]["starting"]==0,30,"split startup")
                        harness.inspect(args.client,address,"key","ctrl+shift+w")
                    until(lambda:frames(log) and frames(log)[-1]["resources"]["running"]==count and frames(log)[-1]["resources"]["starting"]==0 and frames(log)[-1]["resources"]["closing"]==0,30,"rapid lifecycle cleanup")
            latencies=[]
            # End-to-end latency is measured separately from internal frame cost.
            for index in range(5):
                marker=data/f"latency-{index}";tick=time.monotonic()
                command(args.client,address,f"printf ready > {shlex.quote(str(marker))}")
                until(marker.exists,10,"shell latency marker");latencies.append((time.monotonic()-tick)*1000)
            # Probe traffic belongs outside the quiet CPU/RSS interval.
            time.sleep(.25)
            before_parsed={item["pane"]:item["parsed"] for item in frames(log)[-1]["panes"]}
            baseline=proc_sample(process.pid);measure_start=time.monotonic()
            time.sleep(args.seconds)
            elapsed=time.monotonic()-measure_start;after=proc_sample(process.pid);events=frames(log)
            after_parsed={item["pane"]:item["parsed"] for item in events[-1]["panes"]}
            activity={label:after_parsed[int(label.rsplit(' ',1)[1])]-before_parsed[int(label.rsplit(' ',1)[1])] for label in result["output_producers"]}
            result["output_activity_bytes"]=activity
            if any(delta<10_000 for delta in activity.values()):raise RuntimeError("An intended output producer stopped before the CPU/RSS sample completed")
            result.update({"status":"passed","sample_seconds":elapsed,"cpu_core_percent":100*(after["cpu_ticks"]-baseline["cpu_ticks"])/os.sysconf("SC_CLK_TCK")/elapsed,"rss_bytes":after["rss_bytes"],"rss_delta_bytes":after["rss_bytes"]-baseline["rss_bytes"],"threads":after["threads"],"latency_scope":"inspection text/key request to shell file creation; includes inspection roundtrip and readiness polling, not keyboard-to-screen","latency_ms":{"p50":percentile(latencies,.5),"p95":percentile(latencies,.95),"p99":percentile(latencies,.99),"samples":latencies},"frames":events[-1] if events else None})
            harness.inspect(args.client,address,"screenshot",str(output/"native.png"))
            close=next(node for node in tree_nodes(args.client,address) if node["properties"].get("label")=="Close window")
            click(args.client,address,close);process.wait(timeout=8)
            shutdowns=[json.loads(line) for line in log.read_text(errors="replace").splitlines() if line.startswith('{') and '"operation":"shutdown"' in line]
            result["shutdown_complete"]=bool(shutdowns and shutdowns[-1]["complete"])
            if not result["shutdown_complete"]:raise RuntimeError("Session teardown did not complete within shutdown boundary")
    except Exception as error:
        result["status"]="failed";result["error"]=str(error)
    finally:
        if process is not None:harness.stop_owned(process)
        (output/"result.json").write_text(json.dumps(result,indent=2)+'\n')
    print(f"{count:2} panes {scenario}: {result['status']}",flush=True)
    return result

def main():
    parser=argparse.ArgumentParser(description=__doc__,formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--app',type=Path,default=REPO/'target/release/pace')
    parser.add_argument('--client',type=Path,default=REPO/'target/release/pace-inspect')
    parser.add_argument('--counts',type=int,nargs='+',default=[1,8,32,64])
    parser.add_argument('--scenarios',nargs='+',choices=['idle','visible-output','several-output','hidden-output','lifecycle'],default=['idle','visible-output','several-output','hidden-output','lifecycle'])
    parser.add_argument('--seconds',type=float,default=5)
    parser.add_argument('--output',type=Path,default=REPO/'artifacts/scale')
    args=parser.parse_args();args.app=args.app.resolve();args.client=args.client.resolve()
    if not sys.platform.startswith('linux'):parser.error('Process CPU/RSS adapter currently supports Linux only')
    if args.seconds<1 or args.seconds>60 or any(count<1 or count>64 for count in args.counts):parser.error('Use 1..64 panes and 1..60 sample seconds')
    if not args.app.is_file() or not args.client.is_file():parser.error('Build both release inspection binaries first')
    run=datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')+'-'+uuid.uuid4().hex[:8];root=args.output.resolve()/run;root.mkdir(parents=True)
    report={"run_id":run,"platform":platform.platform(),"compiler":subprocess.check_output(['rustc','--version'],text=True).strip(),"features":["inspection"],"app":str(args.app),"binary_sha256":hashlib.sha256(args.app.read_bytes()).hexdigest(),"lock_sha256":hashlib.sha256((REPO/"Cargo.lock").read_bytes()).hexdigest(),"script_sha256":hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),"sample_seconds":args.seconds,"cases":[]}
    for count in args.counts:
        for scenario in args.scenarios:
            # Lifecycle at full capacity releases and restores the same slot.
            report['cases'].append(run_case(args,count,scenario,root));(root/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(f"Measurements: {root/'report.json'}",flush=True)
    return int(any(case['status'] not in ('passed','not-applicable') for case in report['cases']))
if __name__=='__main__':raise SystemExit(main())
