# OOPS Specifications

| Spec ID | Version | Status | Document |
|---|---|---|---|
| `oops-v1` | 1 | normative, frozen | [oops-v1.md](oops-v1.md) |
| `oops-compiler-versioning` | 1 | normative | [compiler-versioning.md](compiler-versioning.md) |

Rules:

1. `oops-vN.md` describes exactly language version `N`.
2. A published `oops-vN.md` MUST NOT change semantics.
3. New language features require `oops-v(N+1).md`.
4. `compiler-versioning.md` governs compiler selection and dependency direction.
