import json,hashlib,sys
O=sys.argv[1]
def q(i,y,n,u): return {"type":"choice","instructions":i,"criteria":{"yes":y,"no":n,"unclear":u}}
B={
"single_constraint":q("Judge only `anchor.statement`. Count the independently enforceable duties it imposes: a duty is independent if one could be satisfied while the other is violated and they govern different subjects or outcomes. A single duty may name several outputs, targets, or conditions (for example stdout, stderr and saved files) and still be one duty. Ignore the quote, scope, vagueness and wording.",
 "The statement imposes exactly one duty; any listed outputs, targets or conditions all serve that one duty.",
 "The statement joins two or more independent duties, such as two different components each required to do a different thing.",
 "It cannot be determined whether the listed parts are one duty or several independent duties."),
"observable":q("Judge only `anchor.statement`. Could a reviewer inspect a concrete behavior, file, output, path or value and decide compliance yes or no? Ignore grammar or sentence form, fidelity to the quote, and bundling.",
 "The statement names a concrete behavior, artifact, path, output or value whose presence or absence can be checked.",
 "Compliance rests only on a subjective quality (for example robust, clean, fast), an undefined label or code (for example R7), or a person's approval with no named record.",
 "A concrete artifact or behavior is named, but what would count as compliance is still not determinable."),
"faithful":q("Compare `anchor.statement` with `anchor.quote`. Ordinary normalization is faithful: an informal request may become must, print may become emit to stdout, machine-readable JSON may become JSON, keeping the user's own second-person phrasing is faithful, and the statement may capture just one of several directives in the quote. Decide whether the statement adds or drops a substantive requirement such as a specific algorithm, strength, trigger, value or outcome that the quote does not state. Do not judge vagueness, testability or grammar.",
 "Every requirement in the statement is stated or directly implied by the quote; only ordinary normalization was applied.",
 "The statement adds a substantive requirement the quote does not contain (for example a specific algorithm, number or mechanism), or changes the quote's meaning.",
 "It cannot be determined whether a detail in the statement goes beyond the quote."),
"scoped":q("Using `anchor.statement` and `anchor.scope`, can a reviewer tell which activity, component, command, artifact or explicit global set (for example all CLI commands) the anchor governs? Judge only whether the area is identifiable, even when the requirement itself is vague or defective.",
 "The governed component, activity, artifact or explicit global set is identifiable.",
 "No governed area or applicability set can be identified.",
 "An area is named but which work it applies to cannot be determined."),
"self_contained":q("Can `anchor.statement` be applied by a reviewer who has no private context? Disqualifiers are only: an undefined subjective quality (for example robust), a private label or code whose meaning is not given (for example R7), or a requirement for a person's approval with no named recorded artifact. Ordinary technical terms, file paths, flags, environment variables, concrete prohibitions and second-person phrasing are not disqualifiers. Ignore grammar, bundling, fidelity and scope.",
 "No disqualifier is present; the statement can be applied using only its own words and ordinary technical knowledge.",
 "At least one decisive term is an undefined subjective quality or private label, or compliance needs an approval with no named record.",
 "A term might depend on outside context, but whether it decides compliance cannot be determined."),
}
C={
"single_constraint":q("Does `anchor.statement` state one invariant? Treat the statement as bundled only when it joins separate obligations on different subjects or with different outcomes (for example 'the CLI must emit JSON and the daemon must write an audit log'). A trigger condition, several output channels, or a list of forbidden values all belong to one invariant. Do not judge any other property.",
 "One invariant: every clause supports a single obligation.",
 "Two or more separate obligations are joined, each enforceable on its own.",
 "The statement's clauses could be read as one obligation or as several."),
"observable":q("Is the outcome required by `anchor.statement` checkable by inspection with a determinate yes/no result? Named files, paths, output streams, formats, values and behaviors under stated conditions are checkable. A subjective adjective, an unexplained label, or a person's say-so without a recorded artifact is not. Judge observability only.",
 "The required outcome is a concrete behavior or artifact that can be checked.",
 "The required outcome is only a subjective quality, an unexplained label, or an unrecorded approval.",
 "An outcome is named but the check that would settle it is ambiguous."),
"faithful":q("Is `anchor.statement` a meaning-preserving restatement of one directive in `anchor.quote`? Allowed: changing informal wording to must, print to emit on stdout, machine-readable JSON to JSON, keeping the quote's own phrasing, and selecting one directive from a quote that contains several. Not allowed: introducing a specific mechanism, algorithm, number, trigger or outcome the quote does not give. Do not judge vagueness or form.",
 "The statement preserves the quote's meaning with only the allowed changes.",
 "The statement introduces a specific mechanism, algorithm, number, trigger or outcome absent from the quote, or alters its meaning.",
 "Whether the statement's details come from the quote cannot be settled."),
"scoped":q("Do `anchor.statement` and `anchor.scope` together identify where the anchor applies: a component, command, activity, artifact, repository location, or an explicit all-items set? Judge identifiability of the area only, not the quality of the requirement.",
 "The applicable area is identifiable.",
 "No applicable area can be identified.",
 "An area is mentioned but its boundaries cannot be determined."),
"self_contained":q("Does `anchor.statement` avoid depending on context outside itself? It depends on outside context only when a decisive term is an undefined subjective quality (robust), a private code or label (R7), or an approval by a person with no identified record. Concrete values, paths, flags, environment variable names, prohibitions and conditions are self-contained. Judge only this property.",
 "The statement is understandable and applicable from its own words.",
 "A decisive term is an undefined quality or private label, or compliance needs an unrecorded approval.",
 "A term may rely on outside context but it is unclear whether it is decisive."),
}
for name,qs in (("B",B),("C",C)):
    d=hashlib.sha256(json.dumps({"questions":qs},sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()).hexdigest()
    json.dump({"questions":qs,"digest":d},open(f"{O}/cand-{name}.json","w"),sort_keys=True,indent=1)
