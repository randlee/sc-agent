;; tree-sitter-python 0.25 has no pattern field on case_clause.
;; The arm is the child case_pattern. @_fn is a helper and is not a subject.
(case_clause
  (case_pattern) @arm)
