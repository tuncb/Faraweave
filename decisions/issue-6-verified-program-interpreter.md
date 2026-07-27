# Issue #6 — verified-program interpreter

Execution uses a flat slot table whose borrowed, aliased, and owned states follow verified edges, with infallible transfers moving values without duplicate resource charges. Selected implementation IDs dispatch directly to scalar kernels while recorded conversion, lift, shape, fan-out, and release metadata controls all remaining behavior. The IR-only public entry points leave existing source evaluator routing unchanged for the separate cutover issue.
