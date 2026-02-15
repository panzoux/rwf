# TWF Rust Port Specifications

This directory contains the complete specification documentation for porting TWF (Two-pane Window Filer) from C#/.NET to Rust.

## Document Overview

### [overview.md](overview.md)
Complete specification overview that ties together all aspects of the Rust port. This document serves as the main entry point for understanding the entire specification.

### [SUMMARY.md](SUMMARY.md)
Executive summary providing a high-level overview of the project goals, architectural approach, and expected outcomes.

### [architect.md](architect.md)
Detailed architectural analysis of the single AppState pattern, including state classification, transition functions, and side effect isolation.

### [design.md](design.md)
Comprehensive design document detailing the implementation of each core responsibility and their interactions.

### [internal_structure.md](internal_structure.md)
Detailed specification of the internal structure of the single AppState, including sub-structure responsibilities and extension strategies.

### [implementation_guide.md](implementation_guide.md)
Step-by-step implementation guide with code examples and project structure recommendations.

## Architectural Approach

The TWF Rust port adopts a **single AppState architecture** that consolidates all mutable state into one central structure. This approach:

- Eliminates consistency issues between distributed state managers
- Simplifies reasoning about application behavior
- Enables pure functions for business logic
- Provides clear separation between state management and side effects
- Facilitates comprehensive testing

## Key Concepts

### State Classification
- **Pure State**: Stored in AppState (filesystem, view, history, config, search, marking)
- **Pure Transition Logic**: Stateless functions that transform state
- **Side Effects**: External interactions (filesystem, terminal, background ops)

### Architecture Layers
```
Side Effects (adapters) → Pure Transition Functions → AppState → UI Rendering
```

## Getting Started

1. Read [overview.md](overview.md) for a complete understanding of the specification
2. Review [architect.md](architect.md) for detailed architectural decisions
3. Study [internal_structure.md](internal_structure.md) for the AppState decomposition strategy
4. Consult [implementation_guide.md](implementation_guide.md) for development guidance
5. Use [design.md](design.md) for detailed component implementations
6. Refer to [SUMMARY.md](SUMMARY.md) for executive-level understanding

## Design Philosophy

The specifications embrace Rust's strengths while avoiding common pitfalls:
- Functional programming principles with pure functions
- Clear separation of concerns
- Memory safety without garbage collection
- Zero-cost abstractions
- Cross-platform compatibility

## Migration Strategy

The architecture supports incremental migration from the original C# implementation:
- Clear boundaries enable component-by-component porting
- Compatible configuration formats
- Preserved feature set with improved implementation
- Backward compatibility maintained