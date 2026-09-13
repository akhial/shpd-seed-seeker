# Fresh c47 / C20B normal-release code inspection

Completed read-only binary analysis after root declared the Linux timing
window closed. No engine, test, build, profiler, benchmark or companion binary
was invoked. Normal freezes and source trees were not modified. Each staged
collector run passed its complete freeze/build/binary identity checks and
recorded command stdout/stderr and end-of-run unchanged-input validation.

Baseline is c47e2f57e524b843c8534de3a92d01f4a0512ea3. The candidate is the new
repeatable experiment's receipt-bound two-file repeated-item transformation.

| Normal matching adapter | Baseline | Candidate |
|---|---|---|
| Executable SHA256 | d08204a1a3c629d68ccb38cbe6c6b4f10b44ec65f5b42624ae8253e6ba4e5063 | 5f3b27899a4551858809b9f0d1184eff760992ad8060ac05e4a40f53b4f64641 |
| `.text` address | 0x9e240 | 0x9e980 |
| `.text` bytes | 1,649,650 | 1,659,890 |
| `.text` SHA256 | 68577a713ba3ed76744d192cbbb7b1b262e7dbb0ff5813f8858012e0d1f74a2f | 12a9fd1b225944c5cee98614bd2c0b34083fdccb8b4d3d2f3b67de92f9cf41a0 |

Neither current `.text` hash equals its historical C18b/C20B counterpart,
despite matching sizes and some entry addresses. No historical function or
profiling address was transferred. All locations below were reached from the
current normal executable's own entry, calls, capture construction and RELATIVE
relocations. The newly reached gate tables and viability entries in fact differ
from the old ones.

## Independently established call path

| Boundary | Baseline address | Candidate address |
|---|---|---|
| ELF entry | 0x9e240 | 0x9e980 |
| C entry wrapper selected by own entry | 0xadb00 | 0xae250 |
| Short trampoline, confirmed call through rdi | 0x9fbc0 | 0xa0310 |
| Rust application main | 0xa2ee0 | 0xa3630 |
| Worker table constructed by main | 0x232170 | 0x2350b0 |
| Worker shim, own table+24 relocation | 0xa2cd0 | 0xa3420 |
| Worker body called with captured references | 0x9fbd0 | 0xa0320 |
| FloorGate table passed with captured plan | 0x2320f0 | 0x235030 |
| Floor callback, own table slot5 | 0x1e8e60 | 0x1eb640 |
| Viability direct target of floor callback | 0x123300 | 0x124870 |

The full chain/evidence is retained in `codegen-{baseline,candidate}-{entry,
wrapper,main,worker-vtable,worker-shim,worker,gate-vtable,gates,viability}-01`,
with each address selection in a separate JSON file. Raw vtable zeros were
resolved through each ELF's own dynamic relocations. Metadata and method bodies
corroborate the QueryPlan identity: drop points to the plan destructor, init
checks required-trinket slot length, selected reads the auto policy, wants reads
the vault flag, and floor dispatches the query viability body.

## Current code facts

| Function | Baseline bytes / instruction rows | Candidate bytes / instruction rows |
|---|---:|---:|
| Main | 18,505 / 3,664 | 18,505 / 3,664 |
| Worker | 6,100 / 1,236 | 6,100 / 1,236 |
| Init gate | 135 / 48 | 135 / 48 |
| Selected-trinket gate | 133 / 46 | 133 / 46 |
| Floor gate | 14 / 6 | 14 / 6 |
| Deferred-vault gate | 328 / 95 | 328 / 95 |
| Wants-vault gate | 5 / 2 | 5 / 2 |
| Viability | 2,154 / 538 | 2,502 / 619 |
| Plan destructor | 285 / 81 | 302 / 85 |

Own vtable metadata proves QueryPlan size104→128, alignment8. Current gate and
worker loads corroborate auto-policy offset72→96, vault flag100→124,
unsatisfiable101→125 and generation depth102→126. The new vector's length is
at88 and pointer at80 in candidate viability. Required-trinket slot length40
and ordinary slots pointer56/length64 remain unchanged. These are observations
for these exact normal x64 builds, not stable Rust ABI guarantees. No profile
DWARF was needed or used for these claims.

Main's stack allocation changes0x6e8→0x6f8. Its plan construction result and
captured plan address move rsp+0x5d0→rsp+0x640, while the timed cursor remains
rsp+0x5a8 and that exact address is captured in both workers. Thus plan/cursor
start separation changes40→152 bytes: **B cannot introduce plan/cursor sharing
of a64-byte line in this adapter**. Live addresses and other objects were not
measured; this is not a general false-sharing exclusion for every consumer.

The worker keeps its0x2f8 frame, saved registers, two lock-xadd sites, chunk256,
allocation-size instructions and internal branch offsets. After explicitly
masking only RIP displacement and external direct targets, its only differences
are three same-width disp8 loads: unsatisfiable0x65→0x7d and depth0x66→0x7e
at two sites. No extra worker-side grouping/hash call, plan clone, allocation
or atomic was introduced. This does not claim external callees or RIP data
identical; those are excluded from the instruction-shape comparison.

Init and floor-wrapper instruction shapes agree. Selection/deferred/wants
change only the corresponding field offsets. Floor still calls viability
out of line: there is no inline-to-out-of-line transition. Both viability
frames remain0x78 with the same saved registers. Candidate0x1248c3 reloads the
plan,0x1248c8 loads vector length at+0x58,0x1248cc tests it, and0x1248cf jumps
to0x1249ff when empty. The group body lies before the existing Hall work and
is skipped for empty groups. The retained Hall register/spill assignments and
branch layout change, so equal frame size does not prove equal cycle cost.

The plan destructor adds the new vector's conditional deallocation path.
Destruction is at service teardown, not inside a matching request's search
timer. The new setup allocation for two or more eligible predicates belongs
in the separate planner-latency measurement, not an attributed cheap search
cost.

## Relation to the controls

The exact cheap depth1 query has one ordinary eligible slot, so grouping
returns before HashMap setup; plan analysis is outside the adapter request
timer. The generated floor reaches completed==target and therefore never calls
the floor gate (`main_world.rs:537`). No pending vault exists. Thus the new
multiplicity loop still executes **zero times per timed cheap seed**.

The early Ghost query has no eligible repeated group and reaches existing
floor callbacks before its depth4 horizon. Its added empty-vector check is
real. That direct cost, indirect retained-Hall/code-placement effects, or
environment/worker-tail variation remain possible explanations of small timing
differences; this inspection does not assign a cause or establish a percentage.
One and eight workers execute the same inspected worker body, but that alone
does not establish equivalent scheduling or memory behavior.

Root reports the new matched release controls as cheap1+0.796%, early1+1.334%,
cheap8+0.400% (2 positive/2 negative pairs), early8−0.678% (1 positive/3 negative).
Those are root's measurement findings, not timings from this inspection.
The older C18b negative cheap8/early8 pairs remain historical evidence; they
are not erased or called noise. The smaller current early8 regression is
likewise retained. No padding, field reordering or other production change is
justified by this code inspection alone.

`summary.json` and per-function normalized diffs retain all current comparison
inputs and exact byte/instruction/frame counts. Address normalization is
explicitly an instruction-shape comparison, not a literal identity gate. All
analysis processes have exited; no background work was created.
