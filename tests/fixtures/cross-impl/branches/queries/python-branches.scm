;; Lite gate: match arms + Refusal returns (PR §9.2)
(case_clause pattern: (_) @arm)

(return_statement
  (call function: (identifier) @_fn) @refusal
  (#eq? @_fn "Refusal"))
