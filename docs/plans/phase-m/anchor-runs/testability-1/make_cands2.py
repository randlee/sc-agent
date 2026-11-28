import json,hashlib,sys
O=sys.argv[1]
A=json.load(open(f"{O}/cand-A.json"))["questions"]
def q(i,y,n,u): return {"type":"choice","instructions":i,"criteria":{"yes":y,"no":n,"unclear":u}}
D=q("Compare `anchor.statement` with `anchor.quote` and decide whether the statement keeps the quote's meaning. These changes are ordinary normalization and keep the meaning: informal wording (needs to, should, do not, I want) becoming must or must never; naming the concrete channel the quote's output already goes to (print, output, logs or errors becoming stdout, stderr or a log file); JSON or machine-readable output becoming JSON or a JSON object; spelling out a concrete case the quote describes (a canary key becoming a specific value); and choosing one directive from a quote that has several. A statement is unfaithful only when it adds a requirement of a different kind that the quote does not ask for, such as a specific algorithm, cipher, standard, number of retries, or new trigger, or when it drops or reverses what the quote asks. Do not judge vagueness, testability or grammar.",
 "The statement asks for the same thing as the quote, with at most the ordinary normalizations listed.",
 "The statement adds a requirement of a different kind (for example a specific algorithm, cipher, standard, count or trigger the quote does not give), or drops or reverses what the quote asks.",
 "A detail in the statement might or might not go beyond the quote, and the quote does not settle it.")
E=q("Would a person who said `anchor.quote` agree that `anchor.statement` says what they asked for? Treat as agreement: restating a request as must or must never; making the quote's output channel concrete (print, output, logs or error output as stdout, stderr or files); JSON as a JSON object; a test scenario restated with its concrete values; one of several directives taken alone. Treat as disagreement: the statement picks a specific method, algorithm, cipher, standard or number the speaker never named, or changes or drops what they asked for. Ignore vagueness and sentence form.",
 "The speaker would agree: the statement asks for what the quote asks, using only the allowed restatements.",
 "The statement commits the speaker to a method, algorithm, cipher, standard or number they never named, or changes or drops their request.",
 "It cannot be told whether the speaker would accept a detail in the statement.")
for name,f in (("D",D),("E",E)):
    qs=dict(A); qs["faithful"]=f
    d=hashlib.sha256(json.dumps({"questions":qs},sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()).hexdigest()
    json.dump({"questions":qs,"digest":d},open(f"{O}/cand-{name}.json","w"),sort_keys=True,indent=1)
