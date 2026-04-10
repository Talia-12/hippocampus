---
name: review-pr
description: Perform a detailed PR review covering architecture, correctness, error handling, performance, type safety, and test coverage.
user-invocable: true
---

# PR Review

Review the specified PR branch in detail. Identify the base branch (usually `main`) and examine all changes.

## Process

1. Gather the full diff and commit history for the PR branch using `jj log -r main::<pr-branch>`.
2. Read all changed/added files in full to understand the context.
3. Read surrounding code as needed to evaluate how changes fit the existing architecture.

## Review Categories

Structure your review under these headings:

### General Overview
Your thoughts on the PR as a whole — what it does, whether the approach is sound, and overall quality.

### Specific Problems
Point out any concrete bugs, logic errors, or issues in specific files with file paths and line numbers. Skip this section if there are none.

### Architecture
How well the changes fit with the existing code architecture of the project. Are the right abstractions used? Is code placed in the right modules?

### Correctness
Whether the PR makes assumptions that are not true of the data model or the rest of the codebase. Look for mismatches between assumptions that different parts of the changeset make.

### Error Handling
Are failure modes explicit and well-propagated, or are there silent swallows / unwrap-happy paths?

### Performance
Missing indexes, unnecessary allocations, unbounded queries, unnecessarily redoing work, etc.

### Type Safety
Does the changeset lean on the type system or work around it? Stringly-typed leaks, newtypes that should exist but don't.

### Test Coverage
Are the interesting edge cases tested, not just the happy path? Any regressions left uncovered?

### Final Thoughts
Any closing remarks or recommendations.
