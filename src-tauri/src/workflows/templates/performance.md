# Performance Optimization Workflow

## Overview
Systematic approach to identifying and resolving performance bottlenecks.

## When to Use
Reference this workflow for optimization: `@~/.claude/workflows/performance.md`

## Rule #1: Measure Before Optimizing
Never optimize based on assumptions. Always profile first.

## Process

### Step 1: Define Performance Goals
- What metric matters? (response time, throughput, memory, CPU)
- What is the current value?
- What is the target value?
- What is acceptable for users?

### Step 2: Profile and Measure
- Use profiling tools appropriate to your stack
- Identify the top 3 bottlenecks (80/20 rule)
- Record baseline metrics before any changes

### Step 3: Analyze Bottlenecks
Common categories:
- **I/O bound**: Database queries, network calls, file operations
- **CPU bound**: Algorithms, data processing, serialization
- **Memory bound**: Large data structures, memory leaks, GC pressure
- **Concurrency**: Lock contention, thread starvation

### Step 4: Optimize
Apply fixes in order of impact (biggest bottleneck first):

#### Database
- Add missing indexes for frequent queries
- Avoid N+1 queries (use JOINs or batch loading)
- Use connection pooling
- Cache frequently-read, rarely-written data

#### Network
- Reduce payload size (pagination, field selection)
- Use compression (gzip/brotli)
- Batch API calls where possible
- Add caching headers

#### Algorithm
- Choose appropriate data structures (hash maps for lookups)
- Avoid unnecessary iterations
- Use lazy evaluation where possible
- Consider space-time tradeoffs

#### Frontend
- Lazy load components and routes
- Debounce expensive event handlers
- Virtualize long lists
- Optimize images and assets

### Step 5: Validate
- Re-measure after each change
- Compare against baseline
- Ensure no functionality regression
- Document the improvement

## Anti-Patterns
- Premature optimization (optimizing before profiling)
- Micro-optimization (optimizing things that don't matter)
- Sacrificing readability for negligible gains
- Caching everything without eviction strategy
