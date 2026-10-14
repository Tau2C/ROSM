# Repository Guidelines & Workflow

## Git Commit & Branching Conventions
- **Commit Frequency & Atomicity**:
  - Commit often.
  - Each commit must be a self-contained, working change (clean compilation and passing tests).
- **Commit Message Format**:
  - Follow Conventional Commits prefixes:
    - `feat:` New features and user-facing capabilities
    - `chore:` Project setup, tooling, build scripts, dependency updates
    - `fix:` Bug fixes and error corrections
    - `test:` Adding, expanding, or updating unit and integration tests
    - `refactor:` Code restructuring without behavior changes
    - `docs:` Documentation and specification updates
- **Branching Strategy**:
  - Create dedicated feature branches for complex or multi-phase features.
  - Use small, atomic commits for small changes and incremental progress.
