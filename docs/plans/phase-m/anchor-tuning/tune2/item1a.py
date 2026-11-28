import json,glob,os,re,collections
W="/private/tmp/claude-501/-Users-randlee-github-sc-agent/14d644ff-101c-48fb-8090-95067f922283/scratchpad"
R="docs/plans/phase-m/anchor-runs"
final=open(W+"/final-runs.txt").read().split()
bead=lambda p: re.search(r"agt-m-\d+",p).group(0)
allruns=collections.defaultdict(list)
for p in sorted(glob.glob(R+"/plan-*-agt-m-*.json")): allruns[bead(p)].append(p)
ans=lambda r:(r.get("answers") or {})
def decide(a,t):
    r,c,k=a.get("relevant"),a.get("addressed"),a.get("contradicted")
    if not(r and c and k): return None
    if r["choice"]=="no" and r["confidence"]>=t: return "not_applicable"
    if k["choice"]=="yes" and k["confidence"]>=t: return "contradicted"
    if r["choice"]=="unclear" or c["choice"]=="unclear" or r["confidence"]<t or c["confidence"]<t: return "underspecified"
    if c["choice"]=="no": return "not_addressed"
    if c["choice"]=="yes" and k["choice"]=="no" and k["confidence"]>=t: return "satisfied"
    return "underspecified"
reask=lambda r:"disagreed" if r.get("unstable") else ("agreed" if (r.get("attempts") or 1)>=2 else "not re-asked")
low=lambda rel: rel["choice"]=="unclear" or rel["confidence"]<0.6
rows=[]
for f in final:
    b=bead(f)
    for res in json.load(open(f))["results"]:
        a=res["anchor_id"]; v=res["verdict"]
        if v=="excluded":
            src=None
            for p in reversed(allruns[b]):
                if os.path.basename(p)>=os.path.basename(f): continue
                for r2 in json.load(open(p))["results"]:
                    if r2["anchor_id"]==a and ans(r2).get("relevant"): src=(p,r2); break
                if src: break
            if not src or not low(ans(src[1])["relevant"]): continue
            p,r2=src; rel=ans(r2)["relevant"]
            rows.append(dict(anchor=a,bead=b,kind="excluded; low in "+os.path.basename(p)[14:21],rel=f'{rel["choice"]} {rel["confidence"]}',reask=reask(r2),at05=decide(ans(r2),0.5)))
        else:
            rel=ans(res).get("relevant")
            if not rel or not low(rel): continue
            rows.append(dict(anchor=a,bead=b,kind=v,rel=f'{rel["choice"]} {rel["confidence"]}',reask=reask(res),at05=decide(ans(res),0.5)))
rows.sort(key=lambda r:(r["anchor"],int(r["bead"].split("-")[-1])))
cnt=collections.Counter(r["anchor"] for r in rows)
L=["| Anchor | Bead | Final cell | relevant | Re-ask | At t=0.5 |","|---|---|---|---|---|---|"]
L+= [f'| {r["anchor"]} | {r["bead"]} | {r["kind"]} | {r["rel"]} | {r["reask"]} | {r["at05"]} |' for r in rows]
L+=["","Count per anchor: "+", ".join(f"{a} {n}" for a,n in sorted(cnt.items(),key=lambda x:(-x[1],x[0])))]
dec=[r for r in rows if r["at05"] and r["at05"]!="underspecified"]
L+=[f"Cells: {len(rows)}. Decided at t=0.5: {len(dec)} of {len(rows)} ("+", ".join(f"{k} {v}" for k,v in collections.Counter(r['at05'] for r in dec).items())+"). Threshold not changed."]
open(W+"/item1a.md","w").write("\n".join(L)); print("\n".join(L[-3:]))
