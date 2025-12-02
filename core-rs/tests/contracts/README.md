# Contract Tests - Protocol Invariant Protection

## Purpose

Contract tests verify **invariants that MUST NEVER BREAK** regardless of implementation changes. They are your defense against:

1. **LLM-induced regression** - AI "optimizations" that break protocol guarantees
2. **Accidental breaking changes** - Well-intentioned refactors that violate contracts
3. **Protocol drift** - Slow erosion of protocol purity over time

## Why "Contract" Tests?

The term "contract test" comes from **Design by Contract** (DbC):

- **Preconditions**: What must be true before operation
- **Postconditions**: What must be true after operation
- **Invariants**: What must ALWAYS be true

In ConceptKernel, contracts include:
- Port allocation: 200 ports per slot
- PID validation: Exact equality, no tolerance
- URN format: `ckp://` scheme with specific structure
- Evidence purity: No implementation details leak into protocol
- Temporal ordering: Temporal parts must be chronological

## Test Structure

Each test follows this pattern:

```rust
/// WHY: [Explain the decision or guarantee]
/// REASON: [Technical rationale]
/// BREAKS: [What breaks if this fails]
/// SACRIFICES: [What you're giving up if you change this]
#[test]
fn invariant_name() {
    // Test the invariant

    // If this test fails:
    // [Clear explanation of what's being violated]
}
```

## Files in This Directory

### `port_allocation_contracts.rs`
**Protects**: Port calculation algorithm, multi-project isolation

**Critical Invariants**:
- 200 ports per project slot (not 100, not 150, exactly 200)
- Port ranges never overlap between slots
- Base port is 56000 (protocol constant)
- Discovery port is always slot_base + 43

**What This Prevents**:
- LLM changing `* 200` to `* 100` to "save memory"
- Port range overlaps causing kernel conflicts
- Breaking Node.js runtime compatibility

### `process_tracker_contracts.rs`
**Protects**: BFO-aligned process tracking, evidence chain integrity

**Critical Invariants**:
- Process URN format: `ckp://Process#{type}-{txId}`
- Temporal parts are chronologically ordered
- Process types are controlled vocabulary (invoke, edge-comm, consensus, broadcast)
- txId format ensures uniqueness

**What This Prevents**:
- LLM "simplifying" URN format
- Temporal part ordering violations (breaks BFO)
- Duplicate txIds (breaks evidence chain)

### `kernel_manager_contracts.rs`
**Protects**: PID validation, kernel lifecycle correctness

**Critical Invariants**:
- PID validation uses EXACT equality (no tolerance window)
- PID:START_TIME format is protocol constant
- Stale PID files must fail validation
- Process start time must come from OS, not PID file

**What This Prevents**:
- LLM adding ±1 second tolerance (allows PID reuse false positives)
- Trusting PID file without OS verification (security issue)
- False positives from stale PID files

### `protocol_purity_contracts.rs`
**Protects**: Protocol language-agnosticism, Rust/Node.js/Python interop

**Critical Invariants**:
- Evidence format excludes implementation details (no toolPath, pid, toolType)
- All references use URNs, not filesystem paths
- JSON uses camelCase (JavaScript convention)
- Error codes are protocol-level (not Rust types)

**What This Prevents**:
- LLM "helpfully" adding toolPath to evidence
- Filesystem paths leaking into protocol
- Rust-specific types breaking Node.js parsing

## Running Contract Tests

### Run all contract tests:
```bash
cd core-rs
cargo test --test 'contracts*'
```

### Run specific contract file:
```bash
cargo test --test port_allocation_contracts
cargo test --test process_tracker_contracts
cargo test --test kernel_manager_contracts
cargo test --test protocol_purity_contracts
```

### Run with verbose output (see all assertions):
```bash
cargo test --test 'contracts*' -- --nocapture
```

## When Contract Tests Fail

### Step 1: Identify What Broke
```
❌ FAILED: port_allocation_200_per_slot_invariant
Expected: 56200
Got: 56100
```

### Step 2: Read Test Comments
Every test has `WHY`, `REASON`, `BREAKS`, `SACRIFICES` comments.

### Step 3: Make Informed Decision

**Question**: "Is my change worth sacrificing this guarantee?"

**If YES**:
1. Update the test with NEW rationale
2. Document what you're sacrificing in commit message
3. Consider bumping protocol version
4. Update CLAUDE.md with the change

**If NO**:
1. Revert your change
2. Fix the code to preserve the invariant
3. Investigate WHY your code violated the contract

### Step 4: Never "Just Make Test Pass"

❌ **WRONG**:
```rust
// Changed test to match my code
assert_eq!(calculate_port(...), 56100); // Made it match bug
```

✅ **RIGHT**:
```rust
// Fixed code to match contract
assert_eq!(calculate_port(...), 56200); // Kept contract, fixed code
```

## Common Failure Patterns

### Pattern 1: "LLM Optimization"
```
Change: * 200 → * 100
Test: port_allocation_200_per_slot_invariant
Reason: LLM tried to "save memory"
Fix: Revert to * 200, explain WHY 200 is needed
```

### Pattern 2: "Helpful Addition"
```
Change: Added "toolPath" field to evidence
Test: evidence_format_excludes_implementation_details
Reason: LLM thought it would be "useful for debugging"
Fix: Remove toolPath, keep protocol pure
```

### Pattern 3: "Tolerance Creep"
```
Change: Added ±1 second tolerance to PID validation
Test: pid_validation_exact_equality_no_tolerance
Reason: LLM thought it would "handle clock drift"
Fix: Remove tolerance, exact equality is correct
```

### Pattern 4: "URN Bypass"
```
Change: Built path manually instead of resolving URN
Test: storage_access_uses_urn_resolution
Reason: LLM thought it was "more efficient"
Fix: Use URN resolution, never build paths
```

## Adding New Contract Tests

### When to Add

Add a contract test when:
1. You discover a critical invariant that's not tested
2. A bug reveals a missing guarantee
3. You add a new protocol-level feature

### How to Add

1. **Document the WHY**:
   ```rust
   /// WHY: [Explain the decision]
   /// REASON: [Technical rationale]
   /// BREAKS: [What breaks if violated]
   /// SACRIFICES: [What you give up if changed]
   ```

2. **Test the invariant**, not the implementation:
   ```rust
   // GOOD: Tests contract
   assert_eq!(port_range_size, 200);

   // BAD: Tests implementation detail
   assert_eq!(function_return_value, 56200);
   ```

3. **Provide clear failure messages**:
   ```rust
   assert_eq!(
       actual, expected,
       "Port allocation changed from 200 to {} - breaks multi-project isolation",
       actual
   );
   ```

4. **Include "If this fails" guidance**:
   ```rust
   // If this test fails:
   // - You changed the port calculation
   // - Multi-project isolation is broken
   // - Revert or document breaking change
   ```

## Property-Based Testing (Future)

Contract tests are currently **example-based** (test specific values). Future enhancement: **property-based testing** with `proptest`:

```rust
proptest! {
    #[test]
    fn all_port_ranges_never_overlap(
        slot1 in 1u32..100,
        slot2 in 1u32..100
    ) {
        // Test thousands of random slot combinations
        if slot1 != slot2 {
            let range1 = calculate_range(slot1);
            let range2 = calculate_range(slot2);
            prop_assert!(!ranges_overlap(range1, range2));
        }
    }
}
```

This would catch edge cases that example tests miss.

## Philosophy

Contract tests embody these principles:

1. **Protocol Over Implementation** - Protect protocol guarantees, not code structure
2. **Explicit Over Implicit** - Document WHY, not just WHAT
3. **Fail Loud** - Clear messages about what's being sacrificed
4. **Trust But Verify** - LLMs are helpful, but contracts catch mistakes

**Remember**: These tests exist because "sometimes LLM goes in and tries to implement a feature and doesn't remember why a choice was made." (Your words)

## Success Metrics

Contract tests are successful if:
1. ✅ LLM-induced breaking changes are caught immediately
2. ✅ Test failures explain WHAT you're sacrificing
3. ✅ You can make informed decisions about tradeoffs
4. ✅ Protocol purity is maintained across implementations

## Questions?

See main testing strategy: `/Users/neoxr/git_ckp/ckp.v1.3.16.rust/core-rs/tests/TESTING-STRATEGY.md`
