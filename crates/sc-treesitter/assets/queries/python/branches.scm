;; tree-sitter-python 0.25 has no pattern field on case_clause.
;; The arm is the child case_pattern.
(case_clause
  (case_pattern) @arm)
