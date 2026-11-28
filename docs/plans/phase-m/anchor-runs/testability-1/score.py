import json,sys,os
O=sys.argv[1]
c=json.load(open('.claude/skills/plan-anchoring/assets/fixtures/testability-calibration.v2.json'))
MAP={'atomicity':'single_constraint'}
res={}
for cand in sys.argv[2:]:
    rows=[];exact=conf=stable=third=0;inc=uns=0;confs=[];ident=True
    for case in c['semantic_cases']:
        d=json.load(open(f"{O}/{cand}/{case['case_id']}.json"))
        if not d.get('success'): print(cand,case['case_id'],'FAIL',d.get('error'));continue
        a=d['data']['anchors'][0];cf=a['confirmations']
        for k,v in case['expected'].items():
            role=MAP.get(k,k);r=cf['resolved'].get(role)
            c1=a['answers'][role];c2=a['answers_second'][role]
            st = role not in cf['unstable']
            if role in cf.get('initial_disagreements',[]): third+=1
            ch=r['choice'] if r else None; cc=r['confidence'] if r else 0
            ok = st and ch==v; cn = ok and cc>=0.6 and min(c1['confidence'],c2['confidence'])>=0.6
            if not st: uns+=1
            elif cc<0.6: inc+=1
            exact+=ok; conf+=cn; stable+=st; confs.append(cc)
            rows.append({"candidate":cand,"case_id":case['case_id'],"role":role,"expected":v,"call_1":[c1['choice'],c1['confidence']],"call_2":[c2['choice'],c2['confidence']],"resolved":ch,"resolved_confidence":cc,"stable":st,"accepted_cell":cn})
    n=len(c['semantic_cases'])*5
    res[cand]={"exact":exact,"confident":conf,"stable":stable,"total":n,"third_call_cells":third,"inconclusive":inc,"unstable":uns,"min_conf":min(confs),"mismatches":[r for r in rows if not r['accepted_cell']],"rows":rows}
    print(cand,exact,conf,stable,n,third,inc,uns,round(min(confs),2))
    for r in res[cand]['mismatches']: print('   ',r['case_id'],r['role'],r['expected'],r['call_1'],r['call_2'])
json.dump(res,open(f"{O}/scores.json","w"),indent=1)
