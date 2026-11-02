# Library UX Reference Notes

## Errors are Canonical Structs (M-ERRORS-CANONICAL-STRUCTS) { #M-ERRORS-CANONICAL-STRUCTS }

Reusable library error contracts should prefer stable, named error types with explicit fields over ad hoc tuples, strings, or opaque wrappers.

## Complex Type Construction has Builders (M-INIT-BUILDER) { #M-INIT-BUILDER }

Types with many optional settings or construction invariants should prefer a builder over long constructors or loosely structured parameter sets.

## Abstractions Don't Visibly Nest (M-ABSTRACTIONS-DONT-NEST) { #M-ABSTRACTIONS-DONT-NEST }

Public types and primitive API surfaces should not expose nested or deeply parametrized types such as `Arc<Mutex<Vec<T>>>`; hide the composition behind a named type so callers work with one concept.
