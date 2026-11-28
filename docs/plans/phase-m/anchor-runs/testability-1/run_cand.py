import json,subprocess,sys,concurrent.futures as cf,os
O,cand=sys.argv[1],sys.argv[2]
c=json.load(open('.claude/skills/plan-anchoring/assets/fixtures/testability-calibration.v2.json'))
os.makedirs(f"{O}/{cand}",exist_ok=True)
def run(case):
    a=case['state']['anchor']; out=f"{O}/{cand}/{case['case_id']}.json"
    if os.path.exists(out):
        try:
            if json.load(open(out)).get('success'): return
        except Exception: pass
    r=subprocess.run(["python3",".claude/skills/plan-anchoring/scripts/jev_judge.py","rubric","--statement",a['statement'],"--quote",a['quote'],"--scope",a['scope'],"--questions",f"{O}/cand-{cand}.json"],capture_output=True,text=True)
    open(out,"w").write(r.stdout)
    if r.stderr: sys.stderr.write(case['case_id']+": "+r.stderr)
with cf.ThreadPoolExecutor(4) as ex: list(ex.map(run,c['semantic_cases']))
